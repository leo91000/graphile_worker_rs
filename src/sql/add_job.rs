use crate::{errors::GraphileWorkerError, JobSpec};
use graphile_worker_job::Job;
use indoc::formatdoc;
use sqlx::{query_as, PgExecutor};
use tracing::info;

/// Add a job to the queue
pub async fn add_job(
    executor: impl for<'e> PgExecutor<'e>,
    escaped_schema: &str,
    identifier: &str,
    payload: serde_json::Value,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError> {
    let sql = formatdoc!(
        r#"
            select * from {escaped_schema}.add_job(
                identifier => $1::text,
                payload => $2::json,
                queue_name => $3::text,
                run_at => $4::timestamptz,
                max_attempts => $5::int,
                job_key => $6::text,
                priority => $7::int,
                flags => $8::text[],
                job_key_mode => $9::text
            );
        "#
    );

    let job_key_mode = spec.job_key_mode().clone().map(|jkm| jkm.to_string());

    let job = query_as(&sql)
        .bind(identifier)
        .bind(&payload)
        .bind(spec.queue_name())
        .bind(spec.run_at())
        .bind(spec.max_attempts())
        .bind(spec.job_key())
        .bind(spec.priority())
        .bind(spec.flags())
        .bind(job_key_mode)
        .fetch_one(executor)
        .await?;

    info!(
        identifier,
        payload = ?payload,
        "Job added to queue"
    );

    Ok(Job::from_db_job(job, identifier.to_string()))
}
