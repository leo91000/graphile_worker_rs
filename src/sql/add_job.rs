use crate::{errors::GraphileWorkerError, JobKeyMode, JobSpec};
use chrono::{DateTime, Utc};
use graphile_worker_job::Job;
use indoc::formatdoc;
use sqlx::{query_as, PgExecutor};
use std::collections::HashMap;
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

    let job = query_as(&sql)
        .bind(identifier)
        .bind(&payload)
        .bind(spec.queue_name())
        .bind(run_at)
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

const BORROWED_BATCH_SERIALIZATION_THRESHOLD: usize = 512;
const FAST_BATCH_INSERT_THRESHOLD: usize = 512;

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

#[derive(serde::Serialize)]
struct FastDbJobSpec<'a> {
    task_id: i32,
    payload: &'a serde_json::Value,
    run_at: Option<DateTime<Utc>>,
    max_attempts: Option<i16>,
    priority: Option<i16>,
}

fn build_fast_db_specs<'a>(
    jobs: &'a [JobToAdd<'a>],
    task_details: &TaskDetails,
    default_run_at: Option<DateTime<Utc>>,
) -> Option<Vec<FastDbJobSpec<'a>>> {
    let mut db_specs = Vec::with_capacity(jobs.len());
    let mut task_ids = HashMap::new();
    for job in jobs {
        if job.spec.queue_name().is_some()
            || job.spec.job_key().is_some()
            || job
                .spec
                .flags()
                .as_ref()
                .is_some_and(|flags| !flags.is_empty())
        {
            return None;
        }

        let task_id = if let Some(task_id) = task_ids.get(job.identifier) {
            *task_id
        } else {
            let task_id = task_details.get_id(job.identifier)?;
            task_ids.insert(job.identifier, task_id);
            task_id
        };

        db_specs.push(FastDbJobSpec {
            task_id,
            payload: &job.payload,
            run_at: job.spec.run_at().or(default_run_at),
            max_attempts: *job.spec.max_attempts(),
            priority: *job.spec.priority(),
        });
    }

    Some(db_specs)
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn add_jobs<'a>(
    executor: impl for<'e> PgExecutor<'e>,
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
    let fast_db_specs = if jobs.len() >= FAST_BATCH_INSERT_THRESHOLD {
        build_fast_db_specs(jobs, task_details, default_run_at)
    } else {
        None
    };

    let db_jobs: Vec<graphile_worker_job::DbJob> = if let Some(db_specs) = fast_db_specs {
        let sql = formatdoc!(
            r#"
                WITH specs AS (
                    SELECT *
                    FROM json_to_recordset($1::json) AS spec(
                        task_id int,
                        payload json,
                        run_at timestamptz,
                        max_attempts int,
                        priority int
                    )
                ),
                notified AS (
                    SELECT pg_notify('jobs:insert', '{{"r":' || random()::text || ',"count":' || $2::int::text || '}}')
                )
                INSERT INTO {escaped_schema}._private_jobs AS jobs (
                    task_id,
                    payload,
                    run_at,
                    max_attempts,
                    priority
                )
                SELECT
                    specs.task_id,
                    coalesce(specs.payload, '{{}}'::json),
                    coalesce(specs.run_at, now()),
                    coalesce(specs.max_attempts, 25),
                    coalesce(specs.priority, 0)
                FROM specs
                CROSS JOIN notified
                RETURNING *
            "#
        );

        let specs_json = serde_json::to_value(&db_specs)?;
        let job_count = i32::try_from(jobs.len()).unwrap_or(i32::MAX);

        query_as(&sql)
            .bind(&specs_json)
            .bind(job_count)
            .fetch_all(executor)
            .await?
    } else {
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

        let specs_json = if jobs.len() >= BORROWED_BATCH_SERIALIZATION_THRESHOLD {
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
            serde_json::to_value(&db_specs)?
        } else {
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
            serde_json::to_value(&db_specs)?
        };

        query_as(&sql)
            .bind(&specs_json)
            .bind(job_key_preserve_run_at)
            .fetch_all(executor)
            .await?
    };

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
