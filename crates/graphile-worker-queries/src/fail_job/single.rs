use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::Result;
use graphile_worker_job::Job;

use super::shared::{single_job_params, FailJobTables};

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn fail_job(
    mut executor: impl DbExecutorArg,
    job: &Job,
    schema: impl Into<Schema>,
    worker_id: &str,
    message: &str,
    replacement_payload: Option<serde_json::Value>,
) -> Result<()> {
    let schema = schema.into();
    let tables = FailJobTables::new(&schema);
    let params = single_job_params(job, worker_id, message, replacement_payload);

    if job.job_queue_id().is_some() {
        return fail_queued_job(&mut executor, &tables, params).await;
    }

    fail_unqueued_job(&mut executor, &tables, params).await
}

async fn fail_unqueued_job(
    executor: &mut impl DbExecutorArg,
    tables: &FailJobTables,
    params: Vec<DbValue>,
) -> Result<()> {
    let jobs = &tables.jobs;
    let sql = formatdoc!(
        r#"
            update {jobs} as jobs
            set
                last_error = $2::text,
                run_at = greatest(now(), run_at) + (exp(least(attempts, 10)) * interval '1 second'),
                locked_by = null,
                locked_at = null,
                payload = coalesce($4::json, jobs.payload)
            where id = $1::bigint and locked_by = $3::text;
        "#
    );

    executor.execute(&sql, params.into()).await?;
    Ok(())
}

async fn fail_queued_job(
    executor: &mut impl DbExecutorArg,
    tables: &FailJobTables,
    params: Vec<DbValue>,
) -> Result<()> {
    let jobs = &tables.jobs;
    let job_queues = &tables.job_queues;
    let sql = formatdoc!(
        r#"
            with j as (
                update {jobs} as jobs
                set
                    last_error = $2::text,
                    run_at = greatest(now(), run_at) + (exp(least(attempts, 10)) * interval '1 second'),
                    locked_by = null,
                    locked_at = null,
                    payload = coalesce($4::json, jobs.payload)
                where id = $1::bigint and locked_by = $3::text
                returning *
            )
            update {job_queues} as job_queues
            set locked_by = null, locked_at = null
            from j
            where job_queues.id = j.job_queue_id and job_queues.locked_by = $3::text;
        "#
    );

    executor.execute(&sql, params.into()).await?;
    Ok(())
}
