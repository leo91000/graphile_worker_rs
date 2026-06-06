use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use futures::FutureExt;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    AfterJobRunContext, BeforeJobRunContext, FailureReason, HookResult, JobFetchContext,
    JobStartContext,
};
use graphile_worker_runtime as runtime;
use graphile_worker_task_handler::TaskHandlerOutcome;
use tracing::{debug, error, info, trace, warn, Instrument, Span};

use super::errors::{ProcessJobError, Redacted, RunJobError};
use super::release::release_job;
use super::WorkerRunner;
use crate::sql::get_job::get_job;
use crate::streams::StreamSource;
use crate::tracing::link_to_job_create_span;

/// Fetches and processes a single job from the queue.
///
/// This function attempts to get a job from the database, process it by
/// finding and executing the appropriate task handler, and then mark it
/// as completed or failed based on the result.
///
/// # Arguments
///
/// * `worker` - The worker instance that provides access to the database and task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` containing:
/// - `Ok(Some(job))` if a job was found and processed
/// - `Ok(None)` if no job was available
/// - `Err(ProcessJobError)` if an error occurred during processing
pub(super) async fn process_one_job(
    worker: &WorkerRunner,
    source: StreamSource,
) -> Result<Option<Job>, ProcessJobError> {
    let now = worker.use_local_time.then(Utc::now);
    let task_details_guard = worker.task_details.read().await;
    let job = get_job(
        &worker.database,
        &task_details_guard,
        &worker.escaped_schema,
        &worker.worker_id,
        &worker.forbidden_flags,
        now,
    )
    .await
    .map_err(|e| {
        error!("Could not get job : {:?}", e);
        e
    })?;
    drop(task_details_guard);

    match job {
        Some(job) => {
            let job = Arc::new(job);

            if !worker.hooks.is_empty() {
                worker
                    .hooks
                    .emit(JobFetchContext {
                        job: job.clone(),
                        worker_id: worker.worker_id.clone(),
                    })
                    .await;
            }

            run_and_release_job(job.clone(), worker, &source).await?;
            Ok(Some(
                Arc::try_unwrap(job).unwrap_or_else(|arc| (*arc).clone()),
            ))
        }
        None => {
            trace!(source = ?source, "No job found");
            Ok(None)
        }
    }
}

/// Executes a job's task handler and then marks the job as completed or failed.
///
/// This function runs the job by finding and executing the appropriate task handler,
/// then releases the job by either marking it as completed (if successful) or
/// failed (if an error occurred).
///
/// # Arguments
///
/// * `job` - The job to process (wrapped in Arc for efficient sharing)
/// * `worker` - The worker instance that provides access to task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` containing:
/// - `Ok(())` if the job was processed and released successfully
/// - `Err(ProcessJobError)` if an error occurred during processing or releasing
pub(super) async fn run_and_release_job(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &StreamSource,
) -> Result<(), ProcessJobError> {
    let (job_result, duration) = if worker.hooks.is_empty() {
        let start = Instant::now();
        let job_result = run_job(job.clone(), worker, source).await;
        (job_result, start.elapsed())
    } else {
        let before_result = worker
            .hooks
            .intercept(BeforeJobRunContext {
                job: job.clone(),
                worker_id: worker.worker_id.clone(),
                payload: job.payload().clone(),
            })
            .await;

        match before_result {
            HookResult::Continue => {
                worker
                    .hooks
                    .emit(JobStartContext {
                        job: job.clone(),
                        worker_id: worker.worker_id.clone(),
                    })
                    .await;

                let start = Instant::now();
                let job_result = run_job(job.clone(), worker, source).await;
                let duration = start.elapsed();

                let result_for_hook = job_result
                    .as_ref()
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"));
                let after_result = worker
                    .hooks
                    .intercept(AfterJobRunContext {
                        job: job.clone(),
                        worker_id: worker.worker_id.clone(),
                        result: result_for_hook,
                        duration,
                    })
                    .await;

                match after_result {
                    HookResult::Continue => (job_result, duration),
                    HookResult::Skip => (Ok(()), duration),
                    HookResult::Fail(msg) => (Err(RunJobError::TaskError(msg)), duration),
                }
            }
            HookResult::Skip => {
                debug!(job_id = job.id(), "Job skipped by before_job_run hook");
                (Ok(()), Duration::ZERO)
            }
            HookResult::Fail(msg) => {
                debug!(
                    job_id = job.id(),
                    "Job failed by before_job_run hook: {}", msg
                );
                (Err(RunJobError::TaskError(msg)), Duration::ZERO)
            }
        }
    };

    release_job(job_result, job.clone(), worker, duration)
        .await
        .map_err(|e| {
            error!("Release job error : {:?}", e);
            e
        })?;
    Ok(())
}

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
///
/// # Arguments
///
/// * `job` - The job to execute
/// * `worker` - The worker instance that provides access to task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` indicating whether the job was successfully executed:
/// - `Ok(())` if the task handler completed successfully
/// - `Err(RunJobError)` if an error occurred during execution
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
async fn run_job(
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

    // Create a context for the task handler
    let worker_ctx = WorkerContext::from_shared_job(
        job.clone(),
        worker.database.clone(),
        worker.escaped_schema.clone(),
        worker.worker_id.clone(),
        worker.extensions.clone(),
        worker.task_details.clone(),
        worker.use_local_time,
    );

    // Get the future that will execute the task
    let task_fut = task_fn(worker_ctx);

    // Start timing the execution
    let start = Instant::now();

    // Set up a shutdown handler that waits for the shutdown signal
    // and then gives tasks time to complete before aborting them
    let mut shutdown_signal = worker.shutdown_signal.clone();
    let grace_period = worker.recovery_config.shutdown_grace_period;
    let shutdown_timeout = async {
        (&mut shutdown_signal).await;
        runtime::sleep(grace_period).await;
    };

    let job_task = std::panic::AssertUnwindSafe(task_fut.instrument(span))
        .catch_unwind()
        .fuse();
    let shutdown_timeout = shutdown_timeout.fuse();
    futures::pin_mut!(job_task, shutdown_timeout);

    // Wait for either the task to complete or the shutdown timeout to occur
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
                Err(e) => Err(RunJobError::TaskPanic(panic_payload_to_string(e))),
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

    // Calculate execution duration
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

    // TODO: Handle batch jobs (vec of futures returned by
    // function)

    Ok(())
}
