use std::num::NonZeroUsize;

use graphile_worker_database::NotificationStream;
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;

use super::StreamSource;

/// Internal data structure for managing the job signal stream.
///
/// This struct holds the state needed to produce job signals from both
/// interval-based polling and PostgreSQL notifications.
pub(super) struct JobSignalStreamData {
    /// Timer for regular polling intervals
    pub(super) interval: runtime::Interval,
    /// Listener for PostgreSQL notifications, if the active driver supports it
    pub(super) pg_listener: Option<NotificationStream>,
    /// Signal that completes when the worker should shut down
    pub(super) shutdown_signal: ShutdownSignal,
    /// Number of jobs to process concurrently
    concurrency: usize,
    /// When a job signal is received, yields multiple items to allow for concurrent processing
    yield_n: Option<(NonZeroUsize, StreamSource)>,
    /// Optional receiver for internal job signals (from LocalQueue)
    pub(super) internal_rx: Option<runtime::Receiver<()>>,
}

impl JobSignalStreamData {
    pub(super) fn new(
        interval: runtime::Interval,
        pg_listener: Option<NotificationStream>,
        shutdown_signal: ShutdownSignal,
        concurrency: usize,
        internal_rx: Option<runtime::Receiver<()>>,
    ) -> Self {
        Self {
            interval,
            pg_listener,
            shutdown_signal,
            concurrency,
            yield_n: None,
            internal_rx,
        }
    }

    pub(super) fn queue_concurrency_yields(&mut self, source: StreamSource) {
        self.yield_n = Some((NonZeroUsize::new(self.concurrency).unwrap(), source));
    }

    pub(super) fn yield_pending_source(&mut self) -> Option<StreamSource> {
        let (n, source) = self.yield_n.take()?;
        if n.get() > 1 {
            let remaining_yields = n.get() - 1;
            self.yield_n = Some((NonZeroUsize::new(remaining_yields).unwrap(), source));
        }
        Some(source)
    }
}

pub(super) enum NextSignal {
    Source(StreamSource),
    InternalClosed,
    PgListenerClosed,
    Shutdown,
}
