use graphile_worker_database::NotificationStream;
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;

use crate::local_queue::LocalQueueSignalReceiver;

use super::JobSignalSource;

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
    /// Optional receiver for LocalQueue job signals
    pub(super) local_queue_rx: Option<LocalQueueSignalReceiver>,
}

impl JobSignalStreamData {
    pub(super) fn new(
        interval: runtime::Interval,
        pg_listener: Option<NotificationStream>,
        shutdown_signal: ShutdownSignal,
        local_queue_rx: Option<LocalQueueSignalReceiver>,
    ) -> Self {
        Self {
            interval,
            pg_listener,
            shutdown_signal,
            local_queue_rx,
        }
    }
}

pub(super) enum NextSignal {
    Source(JobSignalSource),
    LocalQueueClosed,
    NotificationListenerClosed,
    Shutdown,
}
