use std::{fmt, future::Future, time::Duration};

use futures::FutureExt;
use graphile_worker_shutdown_signal::ShutdownSignal;

/// Worker shutdown behavior.
///
/// These options apply to ordinary graceful shutdown, even when worker recovery
/// heartbeat/sweeping is disabled.
#[derive(Clone)]
pub struct WorkerShutdownConfig {
    /// Whether to install OS-level shutdown signal handlers.
    pub listen_os_shutdown_signals: bool,
    /// Time to let in-flight tasks finish after a shutdown signal.
    pub grace_period: Duration,
    /// Delay before a shutdown-aborted job is eligible to run again.
    pub interrupted_job_retry_delay: Duration,
    /// Custom application shutdown signal.
    pub shutdown_signal: Option<ShutdownSignal>,
}

impl Default for WorkerShutdownConfig {
    fn default() -> Self {
        Self {
            listen_os_shutdown_signals: true,
            grace_period: Duration::from_secs(5),
            interrupted_job_retry_delay: Duration::from_secs(30),
            shutdown_signal: None,
        }
    }
}

impl fmt::Debug for WorkerShutdownConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerShutdownConfig")
            .field(
                "listen_os_shutdown_signals",
                &self.listen_os_shutdown_signals,
            )
            .field("grace_period", &self.grace_period)
            .field(
                "interrupted_job_retry_delay",
                &self.interrupted_job_retry_delay,
            )
            .field("shutdown_signal", &self.shutdown_signal.is_some())
            .finish()
    }
}

impl WorkerShutdownConfig {
    pub fn listen_os_shutdown_signals(mut self, value: bool) -> Self {
        self.listen_os_shutdown_signals = value;
        self
    }

    pub fn grace_period(mut self, value: Duration) -> Self {
        self.grace_period = value;
        self
    }

    pub fn interrupted_job_retry_delay(mut self, value: Duration) -> Self {
        self.interrupted_job_retry_delay = value;
        self
    }

    /// Sets a custom application shutdown signal.
    ///
    /// The future should complete when the host application requests shutdown.
    /// The worker still owns graceful draining after the signal is received.
    pub fn shutdown_signal(mut self, signal: impl Future<Output = ()> + Send + 'static) -> Self {
        self.shutdown_signal = Some(future_to_shutdown_signal(signal));
        self
    }
}

pub(crate) fn future_to_shutdown_signal(
    signal: impl Future<Output = ()> + Send + 'static,
) -> ShutdownSignal {
    signal.boxed().shared()
}
