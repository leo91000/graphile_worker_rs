use std::sync::Arc;

use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{JobFailContext, JobPermanentlyFailContext};
use serde_json::Value;
use tracing::{error, warn};

use crate::batcher::FailureRequest;
use crate::sql::fail_job::single::fail_job;

use super::super::errors::{ReleaseJobError, RunJobError};
use super::super::recovery_release::recover_shutdown_aborted_job;
use super::super::WorkerRunner;

pub(super) async fn release_failed_job(
    error: RunJobError,
    job: Arc<Job>,
    worker: &WorkerRunner,
) -> Result<(), ReleaseJobError> {
    if matches!(error, RunJobError::TaskAborted) {
        let recovery_delay = worker.recovery_config.shutdown_recovery_delay;
        recover_shutdown_aborted_job(worker, job, recovery_delay).await?;
        return Ok(());
    }

    let persisted_error = error.persisted_error();
    let will_retry = job.attempts() < job.max_attempts();
    let replacement_payload = error.replacement_payload().cloned();

    log_failed_job(&error, &job, will_retry);

    if replacement_payload.is_none() {
        if let Some(batcher) = &worker.failure_batcher {
            batcher
                .fail(FailureRequest {
                    job,
                    error: persisted_error,
                    will_retry,
                })
                .await;
            return Ok(());
        }
    }

    persist_failed_job(&job, worker, &persisted_error, replacement_payload).await?;
    emit_failure_hook(job, worker, persisted_error, will_retry).await;
    Ok(())
}

fn log_failed_job(error: &RunJobError, job: &Job, will_retry: bool) {
    if will_retry {
        warn!(
            error = ?error,
            task_id = job.task_id(),
            payload = ?job.payload(),
            job_id = job.id(),
            "Failed task"
        );
    } else {
        error!(
            error = ?error,
            task_id = job.task_id(),
            payload = ?job.payload(),
            job_id = job.id(),
            "Job max attempts reached"
        );
    }
}

async fn persist_failed_job(
    job: &Arc<Job>,
    worker: &WorkerRunner,
    error: &str,
    replacement_payload: Option<Value>,
) -> Result<(), ReleaseJobError> {
    if let Err(source) = fail_job(
        &worker.database,
        job,
        &worker.schema,
        &worker.worker_id,
        error,
        replacement_payload,
    )
    .await
    {
        error!(error = ?source, job_id = job.id(), "Failed to persist failed job");
        return Err(ReleaseJobError {
            job_id: *job.id(),
            source,
        });
    }

    Ok(())
}

async fn emit_failure_hook(job: Arc<Job>, worker: &WorkerRunner, error: String, will_retry: bool) {
    if worker.hooks.is_empty() {
        return;
    }

    if will_retry {
        worker
            .hooks
            .emit(JobFailContext {
                job,
                worker_id: worker.worker_id.clone(),
                error,
                will_retry,
            })
            .await;
    } else {
        worker
            .hooks
            .emit(JobPermanentlyFailContext {
                job,
                worker_id: worker.worker_id.clone(),
                error,
            })
            .await;
    }
}
