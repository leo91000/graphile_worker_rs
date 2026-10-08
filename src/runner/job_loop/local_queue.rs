use std::sync::Arc;

use futures::stream::FuturesUnordered;
use graphile_worker_runtime as runtime;
use tracing::{debug, warn};

use super::super::errors::ProcessJobError;
use super::super::{sources, Worker, WorkerRuntimeError};
use crate::local_queue::{LocalQueue, LocalQueueSignalReceiver};
use crate::streams::job_signal::{job_signal_stream, JobSignalStreamConfig};

pub(super) async fn run(
    worker: &Worker,
    local_queues: Vec<LocalQueue>,
    job_signal_rx: LocalQueueSignalReceiver,
) -> Result<(), WorkerRuntimeError> {
    let job_signal = match job_signal_stream(
        JobSignalStreamConfig::new(
            worker.database.clone(),
            worker.poll_interval,
            worker.use_notification_delivery,
            worker.shutdown_signal.clone(),
        )
        .with_local_queue(job_signal_rx),
    )
    .await
    {
        Ok(stream) => stream,
        Err(error) => {
            worker.request_shutdown();
            release_local_queues(&local_queues).await;
            return Err(error.into());
        }
    };

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
            let mut shutdown_signal = runner.shutdown_signal.clone();
            while let Some(source) =
                sources::next_job_signal(&source_rx, &mut shutdown_signal).await
            {
                sources::process_local_queue_source(&runner, &local_queues, index, source).await?;
            }

            Ok::<(), ProcessJobError>(())
        }));
    }
    drop(source_rx);

    let dispatch_result = sources::dispatch_job_signals(
        job_signal,
        source_tx,
        worker_handles,
        worker.concurrency,
        &worker.shutdown_notifier,
    )
    .await;

    release_local_queues(&local_queues).await;

    dispatch_result?;
    Ok(())
}

async fn release_local_queues(local_queues: &[LocalQueue]) {
    for local_queue in local_queues {
        if let Err(e) = local_queue.release().await {
            warn!(error = %e, "Error releasing LocalQueue");
        }
    }
}
