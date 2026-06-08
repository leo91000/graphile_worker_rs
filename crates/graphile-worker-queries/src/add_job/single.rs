use chrono::Utc;
use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use graphile_worker_job::Job;
use graphile_worker_job_spec::JobSpec;
use indoc::formatdoc;
use tracing::info;

use crate::errors::GraphileWorkerError;

use super::super::schema_names::WorkerFunction;

/// Add a job to the queue
#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn add_job(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    identifier: &str,
    payload: serde_json::Value,
    spec: JobSpec,
    use_local_time: bool,
) -> Result<Job, GraphileWorkerError> {
    let add_job = WorkerFunction::AddJob.qualified(schema);
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
