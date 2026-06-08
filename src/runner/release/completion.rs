use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::JobCompleteContext;

use crate::batcher::CompletionRequest;
use graphile_worker_queries::complete_job::complete_job;

use super::super::errors::ReleaseJobError;
use super::super::WorkerRunner;

pub(super) async fn release_completed_job(
    job: Arc<Job>,
    worker: &WorkerRunner,
    duration: Duration,
) -> Result<(), ReleaseJobError> {
    if let Some(batcher) = &worker.completion_batcher {
        batcher
            .complete(CompletionRequest {
                job_id: *job.id(),
                has_queue: job.job_queue_id().is_some(),
                job,
                duration,
            })
            .await;
        return Ok(());
    }

    complete_job(&worker.database, &job, &worker.worker_id, &worker.schema)
        .await
        .map_err(|source| ReleaseJobError {
            job_id: *job.id(),
            source,
        })?;

    emit_completion_hook(job, worker, duration).await;
    Ok(())
}

async fn emit_completion_hook(job: Arc<Job>, worker: &WorkerRunner, duration: Duration) {
    if worker.hooks.is_empty() {
        return;
    }

    worker
        .hooks
        .emit(JobCompleteContext {
            job,
            worker_id: worker.worker_id.clone(),
            duration,
        })
        .await;
}
