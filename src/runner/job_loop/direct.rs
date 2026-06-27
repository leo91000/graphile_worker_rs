use futures::stream::FuturesUnordered;
use graphile_worker_runtime as runtime;
use tracing::debug;

use super::super::errors::ProcessJobError;
use super::super::job_execution::process_one_job;
use super::super::{sources, Worker, WorkerRuntimeError};
use crate::streams::job_signal::{job_signal_stream, JobSignalStreamConfig};

pub(super) async fn run(worker: &Worker) -> Result<(), WorkerRuntimeError> {
    let job_signal = job_signal_stream(JobSignalStreamConfig::new(
        worker.database.clone(),
        worker.poll_interval,
        worker.use_notification_delivery,
        worker.shutdown_signal.clone(),
    ))
    .await?;

    debug!("Listening for jobs...");
    let (source_tx, source_rx) = runtime::channel(worker.concurrency * 4);
    let worker_handles = FuturesUnordered::new();
    let runner = worker.runner();

    for _ in 0..worker.concurrency {
        let runner = runner.clone();
        let source_rx = source_rx.clone();
        worker_handles.push(runtime::spawn(async move {
            while let Ok(source) = source_rx.recv().await {
                let res = process_one_job(&runner, source).await?;

                if let Some(job) = res {
                    debug!(job_id = job.id(), "Job processed");
                }
            }

            Ok::<(), ProcessJobError>(())
        }));
    }
    drop(source_rx);

    sources::dispatch_job_signals(job_signal, source_tx, worker_handles, worker.concurrency)
        .await?;

    Ok(())
}
