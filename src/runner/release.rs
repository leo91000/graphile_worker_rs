use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    JobCompleteContext, JobFailContext, JobPermanentlyFailContext,
};
use tracing::{error, warn};

use super::errors::{ReleaseJobError, RunJobError};
use super::recovery_release::recover_shutdown_aborted_job;
use super::WorkerRunner;
use crate::batcher::{CompletionRequest, FailureRequest};
use crate::sql::complete_job::complete_job;
use crate::sql::fail_job::fail_job;

/// Marks a job as completed or failed based on the result of its execution.
///
/// This function releases a job by either:
/// - Marking it as completed if the task handler executed successfully
/// - Marking it as failed if the task handler encountered an error
///
/// When marking a job as failed, it will be retried later if the maximum
/// number of attempts has not been reached.
///
/// # Arguments
///
/// * `job_result` - The result of executing the job's task handler
/// * `job` - The job that was executed (wrapped in Arc)
/// * `worker` - The worker instance that provides access to the database
/// * `duration` - How long the job took to execute
///
/// # Returns
///
/// A `Result` indicating whether the job was successfully released:
/// - `Ok(())` if the job was marked as completed or failed in the database
/// - `Err(ReleaseJobError)` if an error occurred while updating the database
pub(super) async fn release_job(
    job_result: Result<(), RunJobError>,
    job: Arc<Job>,
    worker: &WorkerRunner,
    duration: Duration,
) -> Result<(), ReleaseJobError> {
    match job_result {
        Ok(_) => {
            if let Some(batcher) = &worker.completion_batcher {
                batcher
                    .complete(CompletionRequest {
                        job_id: *job.id(),
                        has_queue: job.job_queue_id().is_some(),
                        job,
                        duration,
                    })
                    .await;
            } else {
                complete_job(
                    &worker.database,
                    &job,
                    &worker.worker_id,
                    &worker.escaped_schema,
                )
                .await
                .map_err(|e| ReleaseJobError {
                    job_id: *job.id(),
                    source: e,
                })?;

                if !worker.hooks.is_empty() {
                    worker
                        .hooks
                        .emit(JobCompleteContext {
                            job,
                            worker_id: worker.worker_id.clone(),
                            duration,
                        })
                        .await;
                }
            }
        }
        Err(e) => {
            if matches!(e, RunJobError::TaskAborted) {
                let recovery_delay = worker.recovery_config.shutdown_recovery_delay;
                recover_shutdown_aborted_job(worker, job, recovery_delay).await?;
                return Ok(());
            }

            let error_str = e.persisted_error();
            let will_retry = job.attempts() < job.max_attempts();
            let replacement_payload = e.replacement_payload().cloned();

            if let Some(batcher) = &worker.failure_batcher {
                if let Some(replacement_payload) = replacement_payload.clone() {
                    if let Err(e) = fail_job(
                        &worker.database,
                        &job,
                        &worker.escaped_schema,
                        &worker.worker_id,
                        &error_str,
                        Some(replacement_payload),
                    )
                    .await
                    {
                        error!(error = ?e, job_id = job.id(), "Failed to persist failed job");
                        let job_id = *job.id();
                        return Err(ReleaseJobError { job_id, source: e });
                    }

                    if !will_retry {
                        error!(error = ?e, task_id = job.task_id(), payload = ?job.payload(), job_id = job.id(), "Job max attempts reached");
                    } else {
                        warn!(error = ?e, task_id = job.task_id(), payload = ?job.payload(), job_id = job.id(), "Failed task");
                    }

                    if !worker.hooks.is_empty() {
                        if will_retry {
                            worker
                                .hooks
                                .emit(JobFailContext {
                                    job,
                                    worker_id: worker.worker_id.clone(),
                                    error: error_str,
                                    will_retry,
                                })
                                .await;
                        } else {
                            worker
                                .hooks
                                .emit(JobPermanentlyFailContext {
                                    job,
                                    worker_id: worker.worker_id.clone(),
                                    error: error_str,
                                })
                                .await;
                        }
                    }
                    return Ok(());
                }

                if !will_retry {
                    error!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Job max attempts reached"
                    );
                } else {
                    warn!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Failed task"
                    );
                }

                batcher
                    .fail(FailureRequest {
                        job,
                        error: error_str,
                        will_retry,
                    })
                    .await;
            } else {
                if !will_retry {
                    error!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Job max attempts reached"
                    );
                } else {
                    warn!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Failed task"
                    );
                }

                if let Err(e) = fail_job(
                    &worker.database,
                    &job,
                    &worker.escaped_schema,
                    &worker.worker_id,
                    &error_str,
                    replacement_payload,
                )
                .await
                {
                    error!(error = ?e, job_id = job.id(), "Failed to persist failed job");
                    return Err(ReleaseJobError {
                        job_id: *job.id(),
                        source: e,
                    });
                }

                if !worker.hooks.is_empty() {
                    if will_retry {
                        worker
                            .hooks
                            .emit(JobFailContext {
                                job,
                                worker_id: worker.worker_id.clone(),
                                error: error_str,
                                will_retry,
                            })
                            .await;
                    } else {
                        worker
                            .hooks
                            .emit(JobPermanentlyFailContext {
                                job,
                                worker_id: worker.worker_id.clone(),
                                error: error_str,
                            })
                            .await;
                    }
                }
            }
        }
    }

    Ok(())
}
