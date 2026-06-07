use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use graphile_worker_job::Job;
use indoc::formatdoc;

use crate::errors::Result;

use super::shared::{ReturnJobIds, ReturnJobTables};

pub async fn return_jobs(
    mut executor: impl DbExecutorArg,
    jobs: &[Job],
    schema: &Schema,
    worker_id: &str,
) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }

    let ids = ReturnJobIds::from_jobs(jobs);
    let tables = ReturnJobTables::new(schema);

    if ids.contains_queued_jobs() {
        return return_queued_jobs(&mut executor, &tables, worker_id, ids).await;
    }

    return_unqueued_jobs(&mut executor, &tables, worker_id, ids.all).await
}

async fn return_unqueued_jobs(
    executor: &mut impl DbExecutorArg,
    tables: &ReturnJobTables,
    worker_id: &str,
    job_ids: Vec<i64>,
) -> Result<()> {
    let jobs = &tables.jobs;
    let sql = formatdoc!(
        r#"
            update {jobs} as jobs
            set
                attempts = GREATEST(0, attempts - 1),
                locked_by = null,
                locked_at = null
            where id = ANY($2::bigint[])
            and locked_by = $1::text;
        "#
    );

    executor
        .execute(
            &sql,
            vec![
                DbValue::Text(worker_id.to_string()),
                DbValue::I64Array(job_ids),
            ]
            .into(),
        )
        .await?;

    Ok(())
}

async fn return_queued_jobs(
    executor: &mut impl DbExecutorArg,
    tables: &ReturnJobTables,
    worker_id: &str,
    ids: ReturnJobIds,
) -> Result<()> {
    let jobs = &tables.jobs;
    let job_queues = &tables.job_queues;
    let sql = formatdoc!(
        r#"
            with j as (
                update {jobs} as jobs
                set
                    attempts = GREATEST(0, attempts - 1),
                    locked_by = null,
                    locked_at = null
                where id = ANY($2::bigint[])
                and locked_by = $1::text
                returning *
            )
            update {job_queues} as job_queues
            set locked_by = null, locked_at = null
            from j
            where job_queues.id = j.job_queue_id
            and job_queues.locked_by = $1::text
            and j.id = ANY($3::bigint[]);
        "#
    );

    executor
        .execute(
            &sql,
            vec![
                DbValue::Text(worker_id.to_string()),
                DbValue::I64Array(ids.all),
                DbValue::I64Array(ids.queued),
            ]
            .into(),
        )
        .await?;

    Ok(())
}
