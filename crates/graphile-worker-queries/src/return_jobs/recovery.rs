use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_job::Job;
use indoc::formatdoc;

use crate::errors::Result;

use super::shared::{recovery_params, ReturnJobTables};

pub async fn return_job_for_recovery(
    mut executor: impl DbExecutorArg,
    job: &Job,
    schema: impl Into<Schema>,
    worker_id: &str,
    recovery_delay: Option<Duration>,
    last_error: Option<&str>,
) -> Result<()> {
    let schema = schema.into();
    let tables = ReturnJobTables::new(&schema);
    let params = recovery_params(worker_id, job, recovery_delay, last_error);

    if job.job_queue_id().is_some() {
        return return_queued_job_for_recovery(&mut executor, &tables, params).await;
    }

    return_unqueued_job_for_recovery(&mut executor, &tables, params).await
}

async fn return_unqueued_job_for_recovery(
    executor: &mut impl DbExecutorArg,
    tables: &ReturnJobTables,
    params: Vec<graphile_worker_database::DbValue>,
) -> Result<()> {
    let jobs = &tables.jobs;
    let sql = formatdoc!(
        r#"
            update {jobs} as jobs
            set
                attempts = GREATEST(0, attempts - 1),
                locked_by = null,
                locked_at = null,
                run_at = CASE
                    WHEN $3::bigint IS NULL THEN jobs.run_at
                    ELSE GREATEST(jobs.run_at, now() + ($3::bigint * interval '1 millisecond'))
                END,
                last_error = COALESCE($4::text, jobs.last_error),
                updated_at = now()
            where id = $2::bigint
            and locked_by = $1::text;
        "#
    );

    executor.execute(&sql, params.into()).await?;
    Ok(())
}

async fn return_queued_job_for_recovery(
    executor: &mut impl DbExecutorArg,
    tables: &ReturnJobTables,
    params: Vec<graphile_worker_database::DbValue>,
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
                    locked_at = null,
                    run_at = CASE
                        WHEN $3::bigint IS NULL THEN jobs.run_at
                        ELSE GREATEST(jobs.run_at, now() + ($3::bigint * interval '1 millisecond'))
                    END,
                    last_error = COALESCE($4::text, jobs.last_error),
                    updated_at = now()
                where id = $2::bigint
                and locked_by = $1::text
                returning *
            )
            update {job_queues} as job_queues
            set locked_by = null, locked_at = null
            from j
            where job_queues.id = j.job_queue_id
            and job_queues.locked_by = $1::text;
        "#
    );

    executor.execute(&sql, params.into()).await?;
    Ok(())
}
