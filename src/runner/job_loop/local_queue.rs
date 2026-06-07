use std::sync::Arc;

use futures::stream::FuturesUnordered;
use graphile_worker_runtime as runtime;
use tracing::{debug, warn};

use super::super::errors::ProcessJobError;
use super::super::{sources, Worker, WorkerRuntimeError};
use crate::local_queue::LocalQueue;
use crate::streams::job_signal_stream_with_receiver;

pub(super) async fn run(
    worker: &Worker,
    local_queues: Vec<LocalQueue>,
    job_signal_rx: crate::streams::JobSignalReceiver,
) -> Result<(), WorkerRuntimeError> {
    let job_signal = job_signal_stream_with_receiver(
        worker.database.clone(),
        worker.poll_interval,
        worker.shutdown_signal.clone(),
        1,
        job_signal_rx,
    )
    .await?;

    debug!("Listening for jobs with LocalQueue...");
    let (source_tx, source_rx) = runtime::channel(worker.concurrency * 4);
    let worker_handles = FuturesUnordered::new();
    let runner = worker.runner();
    let local_queues = Arc::new(local_queues);

    for index in 0..worker.concurrency {
        let local_queues = local_queues.clone();
        let runner = runner.clone();
        let source_rx = source_rx.clone();
        worker_handles.push(runtime::spawn(async move {
            while let Ok(source) = source_rx.recv().await {
                sources::process_local_queue_source(&runner, &local_queues, index, source).await?;
            }

            Ok::<(), ProcessJobError>(())
        }));
    }
    drop(source_rx);

    let dispatch_result =
        sources::dispatch_job_signals(job_signal, source_tx, worker_handles, worker.concurrency)
            .await;

    for local_queue in local_queues.iter() {
        if let Err(e) = local_queue.release().await {
            warn!(error = %e, "Error releasing LocalQueue");
        }
    }

    dispatch_result?;
    Ok(())
}
