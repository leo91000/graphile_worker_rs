use std::sync::Arc;

use graphile_worker_lifecycle_hooks::JobFetchContext;
use graphile_worker_runtime as runtime;

use super::super::errors::ProcessJobError;
use super::super::job_execution::run_and_release_job;
use super::super::{Worker, WorkerRunner};
use crate::local_queue::{LocalQueue, LocalQueueParams, LocalQueueSignalReceiver};
use crate::streams::job_signal::JobSignalSource;
use crate::Job;

pub(in crate::runner) fn create_local_queues(
    worker: &Worker,
) -> Option<(Vec<LocalQueue>, LocalQueueSignalReceiver)> {
    let config = worker.local_queue_config.as_ref()?;
    let (tx, rx) = runtime::channel(worker.concurrency * 2);
    let queue_count = config.queue_count.min(worker.concurrency);
    let queues = (0..queue_count)
        .map(|_| {
            LocalQueue::new(LocalQueueParams {
                config: config.clone(),
                database: worker.database.clone(),
                schema: worker.schema.clone(),
                worker_id: worker.worker_id.clone(),
                task_details: worker.task_details.clone(),
                poll_interval: worker.poll_interval,
                continuous: true,
                shutdown_signal: Some(worker.shutdown_signal.clone()),
                hooks: worker.hooks.clone(),
                job_signal_sender: tx.clone(),
                use_local_time: worker.use_local_time,
            })
        })
        .collect();

    Some((queues, rx))
}

pub(in crate::runner) async fn process_local_queue_source(
    worker: &WorkerRunner,
    local_queues: &[LocalQueue],
    start_index: usize,
    source: JobSignalSource,
) -> Result<(), ProcessJobError> {
    if matches!(source, JobSignalSource::Notification) {
        local_queues[start_index % local_queues.len()]
            .pulse(1)
            .await;
    }

    let mut source = source;
    loop {
        let job = get_job_from_local_queues(worker, local_queues, start_index).await;

        let Some(job) = job else {
            break;
        };

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
        source = JobSignalSource::LocalQueue;
    }

    Ok(())
}

async fn get_job_from_local_queues(
    worker: &WorkerRunner,
    local_queues: &[LocalQueue],
    start_index: usize,
) -> Option<Job> {
    if !worker.forbidden_flags.is_empty() {
        return local_queues[start_index % local_queues.len()]
            .get_job(&worker.forbidden_flags)
            .await;
    }

    for offset in 0..local_queues.len() {
        let queue_index = (start_index + offset) % local_queues.len();
        if let Some(job) = local_queues[queue_index]
            .get_job(&worker.forbidden_flags)
            .await
        {
            return Some(job);
        }
    }

    None
}
