use chrono::{DateTime, Utc};
use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use graphile_worker_job::Job;
use indoc::formatdoc;
use tracing::info;

use crate::{errors::GraphileWorkerError, JobKeyMode};

use super::super::schema_names::WorkerFunction;
use super::super::task_identifiers::TaskDetails;
use super::types::JobToAdd;

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

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn add_jobs<'a>(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    jobs: &[JobToAdd<'a>],
    task_details: &TaskDetails,
    job_key_preserve_run_at: bool,
    use_local_time: bool,
) -> Result<Vec<Job>, GraphileWorkerError> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    validate_batch_job_key_modes(jobs)?;

    let default_run_at = use_local_time.then(Utc::now);
    let sql = add_jobs_sql(schema);

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
        .map(super::super::rows::db_job_from_row)
        .collect::<std::result::Result<Vec<_>, _>>()?;

    info!(count = db_jobs.len(), "Jobs added to queue in batch");

    Ok(db_jobs
        .into_iter()
        .map(|db_job| {
            let identifier = task_details.get_or_empty(db_job.id(), db_job.task_id());
            Job::from_db_job(db_job, identifier)
        })
        .collect())
}

fn validate_batch_job_key_modes(jobs: &[JobToAdd<'_>]) -> Result<(), GraphileWorkerError> {
    for job in jobs {
        if job
            .spec
            .job_key_mode()
            .as_ref()
            .is_some_and(|mode| matches!(mode, JobKeyMode::UnsafeDedupe))
        {
            return Err(GraphileWorkerError::JobScheduleFailed(
                "UnsafeDedupe job_key_mode is not supported in batch add_jobs".to_string(),
            ));
        }
    }

    Ok(())
}

fn build_batch_specs_json<'a>(
    jobs: &[JobToAdd<'a>],
    default_run_at: Option<DateTime<Utc>>,
) -> serde_json::Result<serde_json::Value> {
    if jobs.len() >= BORROWED_BATCH_SERIALIZATION_THRESHOLD {
        return build_borrowed_batch_specs_json(jobs, default_run_at);
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

fn build_borrowed_batch_specs_json<'a>(
    jobs: &[JobToAdd<'a>],
    default_run_at: Option<DateTime<Utc>>,
) -> serde_json::Result<serde_json::Value> {
    let db_specs: Vec<BorrowedDbJobSpec<'_>> = jobs
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
    serde_json::to_value(&db_specs)
}

fn add_jobs_sql(schema: &Schema) -> String {
    let add_jobs = WorkerFunction::AddJobs.qualified(schema);
    let job_spec = schema.identifier("job_spec");
    formatdoc!(
        r#"
            SELECT * FROM {add_jobs}(
                array(
                    SELECT json_populate_recordset(null::{job_spec}, $1::json)
                ),
                $2::boolean
            );
        "#
    )
}
