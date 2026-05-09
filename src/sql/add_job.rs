use crate::{errors::GraphileWorkerError, JobKeyMode, JobSpec};
use chrono::{DateTime, Utc};
#[cfg(feature = "driver-sqlx")]
use graphile_worker_database::DbError;
use graphile_worker_database::{DbExecutor, DbValue};
use graphile_worker_job::Job;
use indoc::formatdoc;
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
    executor: &impl DbExecutor,
    escaped_schema: &str,
    identifier: &str,
    payload: serde_json::Value,
    spec: JobSpec,
    use_local_time: bool,
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

    let run_at = spec.run_at().or_else(|| use_local_time.then(Utc::now));

    let row = executor
        .fetch_one(
            &sql,
            vec![
                DbValue::Text(identifier.to_string()),
                DbValue::Json(payload.clone()),
                DbValue::TextOpt(spec.queue_name().clone()),
                DbValue::TimestampTzOpt(run_at),
                DbValue::I32Opt((*spec.max_attempts()).map(i32::from)),
                DbValue::TextOpt(spec.job_key().clone()),
                DbValue::I32Opt((*spec.priority()).map(i32::from)),
                DbValue::TextArrayOpt(spec.flags().clone()),
                DbValue::TextOpt(job_key_mode),
            ]
            .into(),
        )
        .await?;
    let job = super::rows::db_job_from_row(&row)?;

    info!(
        identifier,
        payload = ?payload,
        "Job added to queue"
    );

    Ok(Job::from_db_job(job, identifier.to_string()))
}

const BORROWED_BATCH_SERIALIZATION_THRESHOLD: usize = 512;

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

#[derive(serde::Serialize)]
struct BorrowedDbJobSpec<'a> {
    identifier: &'a str,
    payload: &'a serde_json::Value,
    queue_name: Option<&'a str>,
    run_at: Option<DateTime<Utc>>,
    max_attempts: Option<i16>,
    job_key: Option<&'a str>,
    priority: Option<i16>,
    flags: Option<&'a [String]>,
}

fn build_batch_specs_json<'a>(
    jobs: &[JobToAdd<'a>],
    default_run_at: Option<DateTime<Utc>>,
) -> serde_json::Result<serde_json::Value> {
    if jobs.len() >= BORROWED_BATCH_SERIALIZATION_THRESHOLD {
        let db_specs: Vec<BorrowedDbJobSpec> = jobs
            .iter()
            .map(|job| BorrowedDbJobSpec {
                identifier: job.identifier,
                payload: &job.payload,
                queue_name: job.spec.queue_name().as_deref(),
                run_at: job.spec.run_at().or(default_run_at),
                max_attempts: *job.spec.max_attempts(),
                job_key: job.spec.job_key().as_deref(),
                priority: *job.spec.priority(),
                flags: job.spec.flags().as_deref(),
            })
            .collect();
        return serde_json::to_value(&db_specs);
    }

    let db_specs: Vec<DbJobSpec> = jobs
        .iter()
        .map(|job| DbJobSpec {
            identifier: job.identifier.to_string(),
            payload: job.payload.clone(),
            queue_name: job.spec.queue_name().clone(),
            run_at: job.spec.run_at().or(default_run_at),
            max_attempts: *job.spec.max_attempts(),
            job_key: job.spec.job_key().clone(),
            priority: *job.spec.priority(),
            flags: job.spec.flags().clone(),
        })
        .collect();
    serde_json::to_value(&db_specs)
}

fn add_jobs_sql(escaped_schema: &str) -> String {
    formatdoc!(
        r#"
            SELECT * FROM {escaped_schema}.add_jobs(
                array(
                    SELECT json_populate_recordset(null::{escaped_schema}.job_spec, $1::json)
                ),
                $2::boolean
            );
        "#
    )
}

#[cfg(feature = "driver-sqlx")]
async fn add_jobs_sqlx<'a>(
    pool: &sqlx::PgPool,
    escaped_schema: &str,
    jobs: &[JobToAdd<'a>],
    task_details: &TaskDetails,
    job_key_preserve_run_at: bool,
    default_run_at: Option<DateTime<Utc>>,
) -> Result<Vec<Job>, GraphileWorkerError> {
    let sql = add_jobs_sql(escaped_schema);

    let specs_json = build_batch_specs_json(jobs, default_run_at)?;
    let db_jobs: Vec<graphile_worker_job::DbJob> = sqlx::query_as(&sql)
        .bind(&specs_json)
        .bind(job_key_preserve_run_at)
        .fetch_all(pool)
        .await
        .map_err(DbError::from)?;

    Ok(db_jobs
        .into_iter()
        .map(|db_job| {
            let identifier = task_details.get_or_empty(db_job.id(), db_job.task_id());
            Job::from_db_job(db_job, identifier)
        })
        .collect())
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn add_jobs<'a>(
    executor: &impl DbExecutor,
    escaped_schema: &str,
    jobs: &[JobToAdd<'a>],
    task_details: &TaskDetails,
    job_key_preserve_run_at: bool,
    use_local_time: bool,
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

    let default_run_at = use_local_time.then(Utc::now);
    #[cfg(feature = "driver-sqlx")]
    if let Some(pool) = executor.try_sqlx_pool() {
        let result = add_jobs_sqlx(
            pool,
            escaped_schema,
            jobs,
            task_details,
            job_key_preserve_run_at,
            default_run_at,
        )
        .await?;
        info!(count = result.len(), "Jobs added to queue in batch");
        return Ok(result);
    }

    let sql = add_jobs_sql(escaped_schema);

    let specs_json = build_batch_specs_json(jobs, default_run_at)?;
    let db_jobs: Vec<graphile_worker_job::DbJob> = executor
        .fetch_all(
            &sql,
            vec![
                DbValue::Json(specs_json),
                DbValue::Bool(job_key_preserve_run_at),
            ]
            .into(),
        )
        .await?
        .iter()
        .map(super::rows::db_job_from_row)
        .collect::<std::result::Result<Vec<_>, _>>()?;

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
