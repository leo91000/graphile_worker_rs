use graphile_worker_task_handler::{BatchTaskHandler, TaskHandler};
use serde::Serialize;
use tracing::Span;

use super::client::WorkerUtils;
use crate::tracing::add_tracing_info;
use graphile_worker_job::Job;
use graphile_worker_job_spec::JobSpec;
use graphile_worker_queries::add_job::single::add_job as insert_job;
use graphile_worker_queries::errors::GraphileWorkerError;

mod analyze;
mod bulk;
mod hooks;

use hooks::invoke_before_job_schedule;

pub(super) use bulk::{add_jobs, add_raw_jobs};

pub(super) async fn add_job<T: TaskHandler>(
    utils: &WorkerUtils,
    payload: T,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError> {
    let identifier = T::IDENTIFIER;
    let mut payload = serde_json::to_value(payload)?;
    add_tracing_info(&mut payload);

    let span = Span::current();
    span.record("otel.name", identifier);
    span.record("messaging.destination.name", identifier);

    let payload = invoke_before_job_schedule(utils, identifier, payload, &spec).await?;
    insert_job(
        &utils.database,
        &utils.schema,
        identifier,
        payload,
        spec,
        utils.use_local_time,
    )
    .await
}

pub(super) async fn add_raw_job<P>(
    utils: &WorkerUtils,
    identifier: &str,
    payload: P,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError>
where
    P: Serialize,
{
    let mut payload = serde_json::to_value(payload)?;
    add_tracing_info(&mut payload);

    let payload = invoke_before_job_schedule(utils, identifier, payload, &spec).await?;
    insert_job(
        &utils.database,
        &utils.schema,
        identifier,
        payload,
        spec,
        utils.use_local_time,
    )
    .await
}

pub(super) async fn add_batch_job<T: BatchTaskHandler>(
    utils: &WorkerUtils,
    payloads: Vec<T>,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError> {
    let span = Span::current();
    span.record("messaging.batch.message_count", payloads.len());

    if payloads.is_empty() {
        return Err(GraphileWorkerError::JobScheduleFailed(
            "batch job payload must contain at least one item".to_string(),
        ));
    }

    let identifier = T::IDENTIFIER;
    span.record("otel.name", identifier);
    span.record("messaging.destination.name", identifier);

    let mut payload = serde_json::to_value(payloads)?;
    if let Some(items) = payload.as_array_mut() {
        for item in items {
            add_tracing_info(item);
        }
    }

    let payload = invoke_before_job_schedule(utils, identifier, payload, &spec).await?;

    let serde_json::Value::Array(payloads) = payload else {
        return Err(GraphileWorkerError::JobScheduleFailed(
            "before_job_schedule must return a JSON array for batch jobs".to_string(),
        ));
    };

    if payloads.is_empty() {
        return Err(GraphileWorkerError::JobScheduleFailed(
            "batch job payload must contain at least one item".to_string(),
        ));
    }

    insert_job(
        &utils.database,
        &utils.schema,
        identifier,
        serde_json::Value::Array(payloads),
        spec,
        utils.use_local_time,
    )
    .await
}
