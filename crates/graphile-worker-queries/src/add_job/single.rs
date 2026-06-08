use chrono::Utc;
use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use graphile_worker_job::Job;
use graphile_worker_job_spec::JobSpec;
use indoc::formatdoc;
use tracing::{info, Span};

use crate::errors::GraphileWorkerError;
use crate::telemetry::{self, TraceInfo};

use super::super::schema_names::WorkerFunction;

/// Add a job to the queue
#[tracing::instrument(
    skip_all,
    err,
    fields(
        otel.kind = "client",
        db.system = "postgresql",
        messaging.system = "graphile-worker",
        messaging.operation.name = "add_job",
        messaging.destination.name = tracing::field::Empty,
        otel.name = tracing::field::Empty
    )
)]
pub async fn add_job(
    mut executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    identifier: &str,
    payload: serde_json::Value,
    spec: JobSpec,
    use_local_time: bool,
) -> Result<Job, GraphileWorkerError> {
    let schema = schema.into();
    let add_job = WorkerFunction::AddJob.qualified(&schema);
    let sql = formatdoc!(
        r#"
            select * from {add_job}(
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
    let span = Span::current();
    span.record("otel.name", identifier);
    span.record("messaging.destination.name", identifier);

    let trace_info = telemetry::current_trace_info();
    let payload = prepare_payload_for_insert(payload, trace_info.as_ref());

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
    let job = super::super::rows::db_job_from_row(&row)?;

    info!(
        identifier,
        payload = ?payload,
        "Job added to queue"
    );

    Ok(Job::from_db_job(job, identifier.to_string()))
}

fn prepare_payload_for_insert(
    mut payload: serde_json::Value,
    trace_info: Option<&TraceInfo>,
) -> serde_json::Value {
    if let Some(trace_info) = trace_info {
        telemetry::add_tracing_info_with_trace(&mut payload, trace_info);
    }

    payload
}

#[cfg(test)]
mod tests {
    use graphile_worker_database::{BoxFuture, DbError, DbExecutor, DbParams, DbRow, Schema};
    use serde_json::json;

    use super::*;

    type TestFuture<'a, T> = BoxFuture<'a, Result<T, DbError>>;

    struct EmptyExecutor;

    impl DbExecutor for EmptyExecutor {
        fn execute<'a>(&'a self, _sql: &'a str, _params: DbParams) -> TestFuture<'a, u64> {
            Box::pin(async { Ok(0) })
        }

        fn fetch_all<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    #[tokio::test]
    async fn add_job_signature_accepts_various_schema_input_types() {
        let executor = EmptyExecutor;
        let schema_string = String::from("graphile_worker");
        let schema = Schema::from("graphile_worker");

        assert!(add_job(
            &executor,
            "graphile_worker",
            "task",
            serde_json::json!({}),
            JobSpec::default(),
            false,
        )
        .await
        .is_err());
        assert!(add_job(
            &executor,
            schema_string.clone(),
            "task",
            serde_json::json!({}),
            JobSpec::default(),
            false,
        )
        .await
        .is_err());
        assert!(add_job(
            &executor,
            &schema_string,
            "task",
            serde_json::json!({}),
            JobSpec::default(),
            false,
        )
        .await
        .is_err());
        assert!(add_job(
            &executor,
            schema,
            "task",
            serde_json::json!({}),
            JobSpec::default(),
            false,
        )
        .await
        .is_err());
    }

    #[test]
    fn prepare_payload_for_insert_adds_trace_info() {
        let trace_info = TraceInfo {
            flags: 1,
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
        };

        let payload = prepare_payload_for_insert(json!({ "value": true }), Some(&trace_info));

        assert_eq!(payload["_trace"], json!(trace_info));
    }
}
