use chrono::{DateTime, Utc};
use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use graphile_worker_job::Job;
use graphile_worker_job_spec::JobKeyMode;
use indoc::formatdoc;
use tracing::{info, Span};

use crate::errors::GraphileWorkerError;

use super::super::schema_names::WorkerFunction;
use super::super::task_identifiers::TaskDetails;
use super::types::JobToAdd;
use crate::telemetry::{self, TraceInfo};

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

#[tracing::instrument(
    skip_all,
    err,
    fields(
        otel.kind = "client",
        db.system = "postgresql",
        messaging.system = "graphile-worker",
        messaging.operation.name = "add_jobs",
        messaging.batch.message_count = tracing::field::Empty,
        messaging.destination.name = tracing::field::Empty,
        otel.name = tracing::field::Empty
    )
)]
pub async fn add_jobs<'a>(
    mut executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    jobs: &[JobToAdd<'a>],
    task_details: &TaskDetails,
    job_key_preserve_run_at: bool,
    use_local_time: bool,
) -> Result<Vec<Job>, GraphileWorkerError> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    validate_batch_job_key_modes(jobs)?;

    let span = Span::current();
    span.record("messaging.batch.message_count", jobs.len());
    if let Some(identifier) = single_batch_identifier(jobs) {
        span.record("otel.name", identifier);
        span.record("messaging.destination.name", identifier);
    }

    let schema = schema.into();
    let default_run_at = use_local_time.then(Utc::now);
    let sql = add_jobs_sql(&schema);

    let trace_info = telemetry::current_trace_info();
    let specs_json = build_batch_specs_json(jobs, default_run_at, trace_info.as_ref())?;
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

fn single_batch_identifier<'a>(jobs: &[JobToAdd<'a>]) -> Option<&'a str> {
    let first_identifier = jobs.first()?.identifier;
    if jobs.iter().all(|job| job.identifier == first_identifier) {
        return Some(first_identifier);
    }

    None
}

fn build_batch_specs_json<'a>(
    jobs: &[JobToAdd<'a>],
    default_run_at: Option<DateTime<Utc>>,
    trace_info: Option<&TraceInfo>,
) -> serde_json::Result<serde_json::Value> {
    if trace_info.is_none() && jobs.len() >= BORROWED_BATCH_SERIALIZATION_THRESHOLD {
        return build_borrowed_batch_specs_json(jobs, default_run_at);
    }

    let db_specs: Vec<DbJobSpec> = jobs
        .iter()
        .map(|job| {
            let mut payload = job.payload.clone();
            if let Some(trace_info) = trace_info {
                telemetry::add_tracing_info_with_trace(&mut payload, trace_info);
            }

            DbJobSpec {
                identifier: job.identifier.to_string(),
                payload,
                queue_name: job.spec.queue_name().clone(),
                run_at: job.spec.run_at().or(default_run_at),
                max_attempts: *job.spec.max_attempts(),
                job_key: job.spec.job_key().clone(),
                priority: *job.spec.priority(),
                flags: job.spec.flags().clone(),
            }
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

#[cfg(test)]
mod tests {
    use graphile_worker_job_spec::JobSpec;
    use serde_json::json;

    use super::*;

    fn trace_info() -> TraceInfo {
        TraceInfo {
            flags: 1,
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
        }
    }

    #[test]
    fn build_batch_specs_json_adds_trace_info_to_each_job_payload() {
        let trace_info = trace_info();
        let spec = JobSpec::default();
        let jobs = vec![
            JobToAdd {
                identifier: "first",
                payload: json!({ "value": 1 }),
                spec: &spec,
            },
            JobToAdd {
                identifier: "second",
                payload: json!([{ "value": 2 }, "unchanged"]),
                spec: &spec,
            },
        ];

        let specs = build_batch_specs_json(&jobs, None, Some(&trace_info)).unwrap();

        assert_eq!(specs[0]["payload"]["_trace"], json!(trace_info));
        assert_eq!(specs[1]["payload"][0]["_trace"], json!(trace_info));
        assert!(specs[1]["payload"][1].get("_trace").is_none());
    }

    #[test]
    fn build_batch_specs_json_leaves_payload_unchanged_without_trace_info() {
        let spec = JobSpec::default();
        let jobs = vec![JobToAdd {
            identifier: "task",
            payload: json!({ "value": 1 }),
            spec: &spec,
        }];

        let specs = build_batch_specs_json(&jobs, None, None).unwrap();

        assert_eq!(specs[0]["payload"], json!({ "value": 1 }));
    }

    #[test]
    fn single_batch_identifier_returns_identifier_for_same_identifier_batch() {
        let spec = JobSpec::default();
        let jobs = vec![
            JobToAdd {
                identifier: "task",
                payload: json!({ "value": 1 }),
                spec: &spec,
            },
            JobToAdd {
                identifier: "task",
                payload: json!({ "value": 2 }),
                spec: &spec,
            },
        ];

        assert_eq!(single_batch_identifier(&jobs), Some("task"));
    }

    #[test]
    fn single_batch_identifier_returns_none_for_mixed_identifier_batch() {
        let spec = JobSpec::default();
        let jobs = vec![
            JobToAdd {
                identifier: "first",
                payload: json!({ "value": 1 }),
                spec: &spec,
            },
            JobToAdd {
                identifier: "second",
                payload: json!({ "value": 2 }),
                spec: &spec,
            },
        ];

        assert_eq!(single_batch_identifier(&jobs), None);
    }
}
