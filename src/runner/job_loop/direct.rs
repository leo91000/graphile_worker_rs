use std::sync::Arc;

use futures::{stream::FuturesUnordered, FutureExt};
use graphile_worker_runtime as runtime;
use tracing::debug;

use super::super::errors::ProcessJobError;
use super::super::job_execution::process_one_job;
use super::super::{sources, Worker, WorkerRuntimeError};
use crate::streams::job_signal::{job_signal_stream, JobSignalSource, JobSignalStreamConfig};

/// Runs direct slots until a fetch finds no ready work, then waits for another
/// signal. A named release can wake slots even after initial signals are exhausted.
pub(super) async fn run(worker: &Worker) -> Result<(), WorkerRuntimeError> {
    let job_signal = job_signal_stream(JobSignalStreamConfig::new(
        worker.database.clone(),
        worker.poll_interval,
        worker.use_notification_delivery,
        worker.shutdown_signal.clone(),
    ))
    .await?;

    let job_signal = sources::with_queue_wakeups(
        job_signal,
        worker.queue_wakeup.clone(),
        worker.shutdown_signal.clone(),
        Arc::new(Vec::new()),
    );

    debug!("Listening for jobs...");
    let (source_tx, source_rx) = runtime::channel(worker.concurrency * 4);
    let worker_handles = FuturesUnordered::new();
    let runner = worker.runner();

    for _ in 0..worker.concurrency {
        let runner = runner.clone();
        let source_rx = source_rx.clone();
        worker_handles.push(runtime::spawn(async move {
            let mut shutdown_signal = runner.shutdown_signal.clone();
            while let Some(source) =
                sources::next_job_signal(&source_rx, &mut shutdown_signal).await
            {
                let mut source = source;
                loop {
                    // Check before starting another claim. Do not cancel an
                    // in-flight claim whose autocommit may already have run.
                    if shutdown_signal.clone().now_or_never().is_some() {
                        break;
                    }
                    let Some(job) = process_one_job(&runner, source).await? else {
                        break;
                    };
                    debug!(job_id = job.id(), "Job processed");
                    // Any ready job can precede the released named backlog.
                    // Keep draining until a fetch actually finds no ready work.
                    source = JobSignalSource::Polling;
                }
            }

            Ok::<(), ProcessJobError>(())
        }));
    }
    drop(source_rx);

    sources::dispatch_job_signals(
        job_signal,
        source_tx,
        worker_handles,
        worker.concurrency,
        &worker.shutdown_notifier,
    )
    .await?;

    Ok(())
}
