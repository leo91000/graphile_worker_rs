use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{FailureReason, JobInterruptedContext};

use crate::recovery::{apply_job_recovery, JobRecoveryRequest};

use super::{ReleaseJobError, WorkerRunner};

pub(super) async fn recover_shutdown_aborted_job(
    worker: &WorkerRunner,
    job: Arc<Job>,
    recovery_delay: Duration,
) -> Result<(), ReleaseJobError> {
    let outcome = apply_job_recovery(
        &worker.database,
        &worker.schema,
        JobRecoveryRequest {
            hooks: Some(&worker.hooks),
            worker_id: &worker.worker_id,
            job: job.clone(),
            previous_worker_id: &worker.worker_id,
            reason: FailureReason::ShutdownAborted,
            recovery_delay,
        },
    )
    .await
    .map_err(|source| ReleaseJobError {
        job_id: *job.id(),
        source,
    })?;

    if outcome.was_handled() && !worker.hooks.is_empty() {
        worker
            .hooks
            .emit(JobInterruptedContext {
                job,
                worker_id: worker.worker_id.clone(),
                reason: FailureReason::ShutdownAborted,
            })
            .await;
    }

    Ok(())
}
