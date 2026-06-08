use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, DbParams, DbValue, Schema};
use graphile_worker_job::Job;
use indoc::formatdoc;

use crate::duration::duration_as_millis_i64;
use crate::errors::Result;
use crate::rows::get_required;
use crate::schema_names::{PrivateTable, WorkerFunction};

pub async fn recover_dead_worker_jobs(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    worker_ids: &[String],
    recovery_delay: Duration,
) -> Result<i32> {
    if worker_ids.is_empty() {
        return Ok(0);
    }

    let recover_dead_worker_jobs = WorkerFunction::RecoverDeadWorkerJobs.qualified(schema);
    let sql = formatdoc!(
        r#"
            SELECT {recover_dead_worker_jobs}(
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

    get_required(&row, "recovered_count")
}

pub async fn get_locked_jobs_for_recovery(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    worker_ids: &[String],
) -> Result<Vec<Arc<Job>>> {
    if worker_ids.is_empty() {
        return Ok(Vec::new());
    }

    let jobs = PrivateTable::Jobs.qualified(schema);
    let tasks = PrivateTable::Tasks.qualified(schema);
    let sql = formatdoc!(
        r#"
            SELECT jobs.*, tasks.identifier AS task_identifier
            FROM {jobs} AS jobs
            JOIN {tasks} AS tasks ON tasks.id = jobs.task_id
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
            let db_job = crate::rows::db_job_from_row(row)?;
            let task_identifier = get_required(row, "task_identifier")?;
            Ok(Arc::new(Job::from_db_job(db_job, task_identifier)))
        })
        .collect()
}
