use std::sync::Arc;

use futures::{stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use graphile_worker_lifecycle_hooks::JobFetchContext;
use graphile_worker_runtime as runtime;

use super::errors::{ProcessJobError, WorkerRuntimeError};
use super::job_execution::run_and_release_job;
use super::{Worker, WorkerRunner};
use crate::local_queue::{LocalQueue, LocalQueueParams};
use crate::streams::StreamSource;
use crate::Job;

pub(super) fn create_local_queues(
    worker: &Worker,
) -> Option<(Vec<LocalQueue>, crate::streams::JobSignalReceiver)> {
    if let Some(ref config) = worker.local_queue_config {
        let (tx, rx) = runtime::channel(worker.concurrency * 2);
        let queue_count = config.queue_count.min(worker.concurrency);
        let queues = (0..queue_count)
            .map(|_| {
                LocalQueue::new(LocalQueueParams {
                    config: config.clone(),
                    database: worker.database.clone(),
                    escaped_schema: worker.escaped_schema.clone(),
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
    } else {
        None
    }
}

pub(super) async fn dispatch_job_signals<S>(
    job_signal: S,
    source_tx: runtime::Sender<StreamSource>,
    mut worker_handles: FuturesUnordered<runtime::JoinHandle<Result<(), ProcessJobError>>>,
    fanout: usize,
) -> Result<(), WorkerRuntimeError>
where
    S: Stream<Item = StreamSource>,
{
    let job_signal = job_signal.fuse();
    futures::pin_mut!(job_signal);

    loop {
        let next_source = job_signal.next().fuse();
        let worker_done = worker_handles.next().fuse();
        futures::pin_mut!(next_source, worker_done);

        futures::select_biased! {
            worker_result = worker_done => {
                match worker_result {
                    Some(Ok(Ok(()))) => {
                        if worker_handles.is_empty() {
                            break;
                        }
                    }
                    Some(Ok(Err(e))) => {
                        source_tx.close();
                        return Err(e.into());
                    }
                    Some(Err(e)) => {
                        source_tx.close();
                        return Err(e.into());
                    }
                    None => break,
                }
            }
            source = next_source => {
                let Some(source) = source else {
                    break;
                };

                let mut closed = false;
                for _ in 0..fanout {
                    if source_tx.try_send(source).is_err() && source_tx.send(source).await.is_err() {
                        closed = true;
                        break;
                    }
                }
                if closed {
                    break;
                }
            }
        }
    }

    drop(source_tx);

    while let Some(result) = worker_handles.next().await {
        result??;
    }

    Ok(())
}

pub(super) async fn process_local_queue_source(
    worker: &WorkerRunner,
    local_queues: &[LocalQueue],
    start_index: usize,
    source: StreamSource,
) -> Result<(), ProcessJobError> {
    if matches!(source, StreamSource::PgListener) {
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
        source = StreamSource::Internal;
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
