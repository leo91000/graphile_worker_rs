use std::sync::Arc;
use std::time::{Duration, Instant};

use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    AfterJobRunContext, BeforeJobRunContext, HookResult, JobStartContext,
};
use tracing::debug;

use crate::runner::errors::RunJobError;
use crate::runner::task_execution::run_job;
use crate::runner::WorkerRunner;
use crate::streams::job_signal::JobSignalSource;

pub(super) async fn run_job_with_hooks(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &JobSignalSource,
) -> (Result<(), RunJobError>, Duration) {
    if worker.hooks.is_empty() {
        return run_without_hooks(job, worker, source).await;
    }

    match before_job_run(job.clone(), worker).await {
        HookResult::Continue => run_with_after_hook(job, worker, source).await,
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
}

async fn run_without_hooks(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &JobSignalSource,
) -> (Result<(), RunJobError>, Duration) {
    let start = Instant::now();
    let job_result = run_job(job, worker, source).await;
    (job_result, start.elapsed())
}

async fn before_job_run(job: Arc<Job>, worker: &WorkerRunner) -> HookResult {
    worker
        .hooks
        .intercept(BeforeJobRunContext {
            job: job.clone(),
            worker_id: worker.worker_id.clone(),
            payload: job.payload().clone(),
        })
        .await
}

async fn run_with_after_hook(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &JobSignalSource,
) -> (Result<(), RunJobError>, Duration) {
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

    match after_job_run(job, worker, &job_result, duration).await {
        HookResult::Continue => (job_result, duration),
        HookResult::Skip => (Ok(()), duration),
        HookResult::Fail(msg) => (Err(RunJobError::TaskError(msg)), duration),
    }
}

async fn after_job_run(
    job: Arc<Job>,
    worker: &WorkerRunner,
    job_result: &Result<(), RunJobError>,
    duration: Duration,
) -> HookResult {
    let result_for_hook = job_result
        .as_ref()
        .map(|_| ())
        .map_err(|e| format!("{e:?}"));

    worker
        .hooks
        .intercept(AfterJobRunContext {
            job,
            worker_id: worker.worker_id.clone(),
            result: result_for_hook,
            duration,
        })
        .await
}
