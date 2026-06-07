mod select;
mod state;

use std::time::Duration;

use futures::{stream, Stream};
use graphile_worker_database::Database;
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::warn;

use crate::errors::Result;
use state::{JobSignalStreamData, NextSignal};

/// Indicates the source of a job signal that triggered job processing.
///
/// This enum represents the different mechanisms that can trigger
/// the worker to check for and process jobs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StreamSource {
    /// Job processing was triggered by a regular polling interval
    Polling,
    /// Job processing was triggered by a PostgreSQL notification
    PgListener,
    /// Job processing came from a one-time run request
    RunOnce,
    /// Job processing was triggered internally (e.g., after jobs were fetched into cache)
    Internal,
}

/// Sender for injecting job signals into the stream.
/// Used by LocalQueue to signal when jobs are available in the cache.
pub type JobSignalSender = runtime::Sender<()>;

/// Receiver for job signals from LocalQueue.
pub type JobSignalReceiver = runtime::Receiver<()>;

/// Creates a stream that yields job processing signals from multiple sources.
///
/// This function returns a stream that emits signals when the worker should check for jobs.
/// The signals come from:
/// 1. Regular interval-based polling (every `poll_interval`)
/// 2. PostgreSQL notifications when new jobs are inserted (`NOTIFY 'jobs:insert'`)
///
/// When a signal is received, the stream will emit enough items to utilize the
/// configured concurrency (one item per potential concurrent job).
///
/// The stream will terminate when the shutdown signal is triggered.
pub async fn job_signal_stream(
    database: Database,
    poll_interval: Duration,
    shutdown_signal: ShutdownSignal,
    concurrency: usize,
) -> Result<impl Stream<Item = StreamSource>> {
    job_signal_stream_internal(database, poll_interval, shutdown_signal, concurrency, None).await
}

/// Creates a stream that yields job processing signals, with a provided receiver for internal signaling.
///
/// This variant is used with LocalQueue when the sender/receiver pair is created externally
/// (e.g., in builder.rs) to avoid race conditions.
pub async fn job_signal_stream_with_receiver(
    database: Database,
    poll_interval: Duration,
    shutdown_signal: ShutdownSignal,
    concurrency: usize,
    internal_rx: JobSignalReceiver,
) -> Result<impl Stream<Item = StreamSource>> {
    job_signal_stream_internal(
        database,
        poll_interval,
        shutdown_signal,
        concurrency,
        Some(internal_rx),
    )
    .await
}

async fn job_signal_stream_internal(
    database: Database,
    poll_interval: Duration,
    shutdown_signal: ShutdownSignal,
    concurrency: usize,
    internal_rx: Option<runtime::Receiver<()>>,
) -> Result<impl Stream<Item = StreamSource>> {
    let interval = runtime::interval(poll_interval);
    let pg_listener = database.listen("jobs:insert").await?;
    let stream_data = JobSignalStreamData::new(
        interval,
        pg_listener,
        shutdown_signal,
        concurrency,
        internal_rx,
    );
    let stream = stream::unfold(stream_data, |mut data| async {
        if let Some(source) = data.yield_pending_source() {
            return Some((source, data));
        }

        match select::next_signal(&mut data).await {
            NextSignal::Source(source) => {
                data.queue_concurrency_yields(source);
                Some((source, data))
            }
            NextSignal::InternalClosed => {
                warn!("Job signal stream internal channel closed");
                None
            }
            NextSignal::PgListenerClosed => {
                data.pg_listener = None;
                let source = StreamSource::Polling;
                data.queue_concurrency_yields(source);
                Some((source, data))
            }
            NextSignal::Shutdown => None,
        }
    });

    Ok(stream)
}
