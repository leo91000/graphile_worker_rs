use std::time::Duration;

/// Worker shutdown behavior.
///
/// These options apply to ordinary graceful shutdown, even when worker recovery
/// heartbeat/sweeping is disabled.
#[derive(Debug, Clone)]
pub struct WorkerShutdownConfig {
    /// Whether to install OS-level shutdown signal handlers.
    pub listen_os_shutdown_signals: bool,
    /// Time to let in-flight tasks finish after a shutdown signal.
    pub grace_period: Duration,
    /// Delay before a shutdown-aborted job is eligible to run again.
    pub interrupted_job_retry_delay: Duration,
}

impl Default for WorkerShutdownConfig {
    fn default() -> Self {
        Self {
            listen_os_shutdown_signals: true,
            grace_period: Duration::from_secs(5),
            interrupted_job_retry_delay: Duration::from_secs(30),
        }
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
}
