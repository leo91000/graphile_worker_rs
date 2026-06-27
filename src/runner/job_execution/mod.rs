mod hooks;

use std::sync::Arc;

use chrono::Utc;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::JobFetchContext;
use tracing::{error, trace};

use self::hooks::run_job_with_hooks;
use super::errors::ProcessJobError;
use super::release::release_job;
use super::WorkerRunner;
use crate::streams::job_signal::JobSignalSource;
use graphile_worker_queries::get_job::get_job;

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
    source: JobSignalSource,
) -> Result<Option<Job>, ProcessJobError> {
    let now = worker.use_local_time.then(Utc::now);
    let task_details_guard = worker.task_details.read().await;
    let job = get_job(
        &worker.database,
        &task_details_guard,
        &worker.schema,
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
    source: &JobSignalSource,
) -> Result<(), ProcessJobError> {
    let (job_result, duration) = run_job_with_hooks(job.clone(), worker, source).await;

    release_job(job_result, job.clone(), worker, duration)
        .await
        .map_err(|e| {
            error!("Release job error : {:?}", e);
            e
        })?;
    Ok(())
}
