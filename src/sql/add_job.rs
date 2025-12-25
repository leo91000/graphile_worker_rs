use crate::{errors::GraphileWorkerError, JobKeyMode, JobSpec};
use chrono::{DateTime, Utc};
use graphile_worker_job::Job;
use indoc::formatdoc;
use sqlx::{query_as, PgExecutor};
use tracing::info;

use super::task_identifiers::TaskDetails;

#[derive(Debug, Clone)]
pub struct RawJobSpec {
    pub identifier: String,
    pub payload: serde_json::Value,
    pub spec: JobSpec,
}

pub struct JobToAdd<'a> {
    pub identifier: &'a str,
    pub payload: serde_json::Value,
    pub spec: &'a JobSpec,
}

/// Add a job to the queue
#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
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

#[derive(serde::Serialize)]
struct DbJobSpec {
    identifier: String,
    payload: serde_json::Value,
    queue_name: Option<String>,
    run_at: Option<DateTime<Utc>>,
    max_attempts: Option<i16>,
    job_key: Option<String>,
    priority: Option<i16>,
    flags: Option<Vec<String>>,
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn add_jobs<'a>(
    executor: impl for<'e> PgExecutor<'e>,
    escaped_schema: &str,
    jobs: &[JobToAdd<'a>],
    task_details: &TaskDetails,
    job_key_preserve_run_at: bool,
) -> Result<Vec<Job>, GraphileWorkerError> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    for job in jobs {
        if let Some(mode) = job.spec.job_key_mode() {
            if matches!(mode, JobKeyMode::UnsafeDedupe) {
                return Err(GraphileWorkerError::JobScheduleFailed(
                    "UnsafeDedupe job_key_mode is not supported in batch add_jobs".to_string(),
                ));
            }
        }
    }

    let db_specs: Vec<DbJobSpec> = jobs
        .iter()
        .map(|job| DbJobSpec {
            identifier: job.identifier.to_string(),
            payload: job.payload.clone(),
            queue_name: job.spec.queue_name().clone(),
            run_at: *job.spec.run_at(),
            max_attempts: *job.spec.max_attempts(),
            job_key: job.spec.job_key().clone(),
            priority: *job.spec.priority(),
            flags: job.spec.flags().clone(),
        })
        .collect();

    let sql = formatdoc!(
        r#"
            SELECT * FROM {escaped_schema}.add_jobs(
                array(
                    SELECT json_populate_recordset(null::{escaped_schema}.job_spec, $1::json)
                ),
                $2::boolean
            );
        "#
    );

    let specs_json = serde_json::to_value(&db_specs)?;

    let db_jobs: Vec<graphile_worker_job::DbJob> = query_as(&sql)
        .bind(&specs_json)
        .bind(job_key_preserve_run_at)
        .fetch_all(executor)
        .await?;

    info!(count = db_jobs.len(), "Jobs added to queue in batch");

    let result: Vec<Job> = db_jobs
        .into_iter()
        .map(|db_job| {
            let identifier = task_details.get_or_empty(db_job.id(), db_job.task_id());
            Job::from_db_job(db_job, identifier)
        })
        .collect();

    Ok(result)
}
