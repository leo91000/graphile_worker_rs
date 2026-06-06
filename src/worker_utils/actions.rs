use graphile_worker_database::{DbExecutor, DbValue};
use indoc::formatdoc;

use super::{RescheduleJobOptions, WorkerUtils};
use crate::sql::dynamic::{DynamicSchema, WorkerFunction};
use crate::{errors::GraphileWorkerError, DbJob};

pub(super) async fn remove_job(
    utils: &WorkerUtils,
    job_key: &str,
) -> Result<(), GraphileWorkerError> {
    let remove_job = DynamicSchema::new(&utils.escaped_schema).function(WorkerFunction::RemoveJob);
    let sql = formatdoc!(
        r#"
            select * from {remove_job}($1::text);
        "#
    );

    utils
        .database
        .execute(&sql, vec![DbValue::Text(job_key.to_string())].into())
        .await?;

    Ok(())
}

pub(super) async fn complete_jobs(
    utils: &WorkerUtils,
    ids: &[i64],
) -> Result<Vec<DbJob>, GraphileWorkerError> {
    let complete_jobs =
        DynamicSchema::new(&utils.escaped_schema).function(WorkerFunction::CompleteJobs);
    let sql = formatdoc!(
        r#"
            select * from {complete_jobs}($1::bigint[]);
        "#
    );

    fetch_db_jobs(utils, &sql, vec![DbValue::I64Array(ids.to_vec())]).await
}

pub(super) async fn permanently_fail_jobs(
    utils: &WorkerUtils,
    ids: &[i64],
    reason: &str,
) -> Result<Vec<DbJob>, GraphileWorkerError> {
    let permanently_fail_jobs =
        DynamicSchema::new(&utils.escaped_schema).function(WorkerFunction::PermanentlyFailJobs);
    let sql = formatdoc!(
        r#"
            select * from {permanently_fail_jobs}($1::bigint[], $2::text);
        "#
    );

    fetch_db_jobs(
        utils,
        &sql,
        vec![
            DbValue::I64Array(ids.to_vec()),
            DbValue::Text(reason.to_string()),
        ],
    )
    .await
}

pub(super) async fn reschedule_jobs(
    utils: &WorkerUtils,
    ids: &[i64],
    options: RescheduleJobOptions,
) -> Result<Vec<DbJob>, GraphileWorkerError> {
    let reschedule_jobs =
        DynamicSchema::new(&utils.escaped_schema).function(WorkerFunction::RescheduleJobs);
    let sql = formatdoc!(
        r#"
            select * from {reschedule_jobs}(
                $1::bigint[],
                run_at := $2::timestamptz,
                priority := $3::int,
                attempts := $4::int,
                max_attempts := $5::int
            );
        "#
    );

    fetch_db_jobs(
        utils,
        &sql,
        vec![
            DbValue::I64Array(ids.to_vec()),
            DbValue::TimestampTzOpt(options.run_at),
            DbValue::I32Opt(options.priority.map(i32::from)),
            DbValue::I32Opt(options.attempts.map(i32::from)),
            DbValue::I32Opt(options.max_attempts.map(i32::from)),
        ],
    )
    .await
}

pub(super) async fn force_unlock_workers(
    utils: &WorkerUtils,
    worker_ids: &[&str],
) -> Result<(), GraphileWorkerError> {
    let force_unlock_workers =
        DynamicSchema::new(&utils.escaped_schema).function(WorkerFunction::ForceUnlockWorkers);
    let sql = formatdoc!(
        r#"
            select * from {force_unlock_workers}($1::text[]);
        "#
    );

    utils
        .database
        .execute(
            &sql,
            vec![DbValue::TextArray(
                worker_ids
                    .iter()
                    .map(|worker_id| worker_id.to_string())
                    .collect(),
            )]
            .into(),
        )
        .await?;

    Ok(())
}

async fn fetch_db_jobs(
    utils: &WorkerUtils,
    sql: &str,
    params: Vec<DbValue>,
) -> Result<Vec<DbJob>, GraphileWorkerError> {
    let jobs = utils
        .database
        .fetch_all(sql, params.into())
        .await?
        .iter()
        .map(crate::sql::rows::db_job_from_row)
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(jobs)
}
