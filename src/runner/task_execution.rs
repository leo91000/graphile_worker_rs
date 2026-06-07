use std::sync::Arc;
use std::time::Instant;

use futures::FutureExt;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::FailureReason;
use graphile_worker_runtime as runtime;
use graphile_worker_task_handler::TaskHandlerOutcome;
use tracing::{debug, info, warn, Instrument, Span};

use super::errors::{Redacted, RunJobError};
use super::WorkerRunner;
use crate::streams::StreamSource;
use crate::tracing::link_to_job_create_span;

pub(super) fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    "task panicked".to_string()
}

/// Executes a job's task handler function.
///
/// This function looks up the appropriate task handler for the job, creates
/// a context with the job's payload and other information, and executes the
/// handler. It also handles shutdown signals gracefully, allowing tasks a
/// timeout period to complete before aborting them.
#[tracing::instrument(
    "run_job",
    skip(job, worker, source),
    fields(
        job_id = job.id(),
        messaging.system = "graphile-worker",
        messaging.operation.name = "run_job",
        messaging.destination.name = tracing::field::Empty,
        otel.name = tracing::field::Empty
    )
)]
pub(super) async fn run_job(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &StreamSource,
) -> Result<(), RunJobError> {
    link_to_job_create_span(job.payload());
    let task_id = job.task_id();
    let task_identifier = job.task_identifier();
    if task_identifier.is_empty() {
        return Err(RunJobError::IdentifierNotFound(*task_id));
    }

    let span = Span::current();
    span.record("otel.name", task_identifier.as_str());
    span.record("messaging.destination.name", task_identifier.as_str());

    let task_fn = worker
        .jobs
        .get(task_identifier)
        .ok_or_else(|| RunJobError::FnNotFound(task_identifier.clone()))?;

    debug!(source = ?source, job_id = job.id(), task_identifier, task_id, "Found task");

    let worker_ctx = WorkerContext::from_shared_job(
        job.clone(),
        worker.database.clone(),
        worker.schema.clone(),
        worker.worker_id.clone(),
        worker.extensions.clone(),
        worker.task_details.clone(),
        worker.use_local_time,
    );

    let task_fut = task_fn(worker_ctx);
    let start = Instant::now();

    let mut shutdown_signal = worker.shutdown_signal.clone();
    let grace_period = worker.shutdown_config.grace_period;
    let shutdown_timeout = async {
        (&mut shutdown_signal).await;
        runtime::sleep(grace_period).await;
    };

    let job_task = std::panic::AssertUnwindSafe(task_fut.instrument(span))
        .catch_unwind()
        .fuse();
    let shutdown_timeout = shutdown_timeout.fuse();
    futures::pin_mut!(job_task, shutdown_timeout);

    futures::select_biased! {
        res = job_task => {
            match res {
                Ok(TaskHandlerOutcome::Complete) => Ok(()),
                Ok(TaskHandlerOutcome::Failed {
                    error,
                    replacement_payload: Some(replacement_payload),
                }) => Err(RunJobError::TaskErrorWithReplacement {
                    message: error,
                    replacement_payload: Redacted::new(replacement_payload),
                }),
                Ok(TaskHandlerOutcome::Failed {
                    error,
                    replacement_payload: None,
                }) => Err(RunJobError::TaskError(error)),
                Err(error) => Err(RunJobError::TaskPanic(panic_payload_to_string(error))),
            }
        }
        _ = shutdown_timeout => {
            let payload = job.payload().to_string();
            warn!(
                task_identifier,
                payload,
                job_id = job.id(),
                grace_period_ms = grace_period.as_millis(),
                reason = ?FailureReason::ShutdownAborted,
                "Job interrupted by shutdown signal"
            );
            Err(RunJobError::TaskAborted)
        }
    }?;

    let duration = start.elapsed();

    if tracing::enabled!(tracing::Level::INFO) {
        let payload = job.payload().to_string();
        info!(
            task_identifier,
            payload,
            job_id = job.id(),
            duration = duration.as_millis(),
            "Completed task with success"
        );
    }

    // TODO: Handle batch jobs (vec of futures returned by function)
    Ok(())
}
