mod completion;
mod failure;

use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;

use super::errors::{ReleaseJobError, RunJobError};
use super::WorkerRunner;

/// Marks a job as completed or failed based on the result of its execution.
///
/// This function releases a job by either:
/// - Marking it as completed if the task handler executed successfully
/// - Marking it as failed if the task handler encountered an error
///
/// When marking a job as failed, it will be retried later if the maximum
/// number of attempts has not been reached.
pub(super) async fn release_job(
    job_result: Result<(), RunJobError>,
    job: Arc<Job>,
    worker: &WorkerRunner,
    duration: Duration,
) -> Result<(), ReleaseJobError> {
    match job_result {
        Ok(()) => completion::release_completed_job(job, worker, duration).await,
        Err(error) => failure::release_failed_job(error, job, worker).await,
    }
}
