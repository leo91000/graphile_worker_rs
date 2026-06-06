use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, DbParams, DbValue};
use graphile_worker_job::Job;
use indoc::formatdoc;

use crate::errors::Result;
use crate::sql::duration::duration_as_millis_i64;

pub async fn recover_dead_worker_jobs(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_ids: &[String],
    recovery_delay: Duration,
) -> Result<i32> {
    if worker_ids.is_empty() {
        return Ok(0);
    }

    let sql = formatdoc!(
        r#"
            SELECT {escaped_schema}.recover_dead_worker_jobs(
                $1::text[],
                $2::bigint * interval '1 millisecond'
            ) AS recovered_count;
        "#
    );

    let row = executor
        .fetch_one(
            &sql,
            DbParams::from(vec![
                DbValue::TextArray(worker_ids.to_vec()),
                DbValue::I64(duration_as_millis_i64(recovery_delay)),
            ]),
        )
        .await?;

    row.try_get("recovered_count").map_err(Into::into)
}

pub async fn get_locked_jobs_for_recovery(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_ids: &[String],
) -> Result<Vec<Arc<Job>>> {
    if worker_ids.is_empty() {
        return Ok(Vec::new());
    }

    let sql = formatdoc!(
        r#"
            SELECT jobs.*, tasks.identifier AS task_identifier
            FROM {escaped_schema}._private_jobs AS jobs
            JOIN {escaped_schema}._private_tasks AS tasks ON tasks.id = jobs.task_id
            WHERE jobs.locked_by = ANY($1::text[])
            ORDER BY jobs.id ASC;
        "#
    );

    let rows = executor
        .fetch_all(
            &sql,
            DbParams::from(vec![DbValue::TextArray(worker_ids.to_vec())]),
        )
        .await?;

    rows.iter()
        .map(|row| {
            let db_job = crate::sql::rows::db_job_from_row(row)?;
            let task_identifier = row.try_get("task_identifier")?;
            Ok(Arc::new(Job::from_db_job(db_job, task_identifier)))
        })
        .collect()
}
