use std::time::Duration;

use super::WorkerOptions;

impl WorkerOptions {
    /// Configures dead worker recovery.
    ///
    /// See [`crate::WorkerRecoveryConfig`] for Pro-aligned options such as
    /// `heartbeat_interval` (`heartbeatInterval`), `sweep_interval` (`sweepInterval`),
    /// and `sweep_threshold` (`sweepThreshold`).
    pub fn worker_recovery(mut self, config: crate::recovery::WorkerRecoveryConfig) -> Self {
        self.worker_recovery_config = Some(config);
        self
    }

    /// Sets the worker heartbeat interval (`heartbeatInterval`).
    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        let mut config = self.worker_recovery_config.unwrap_or_default();
        config.heartbeat_interval = interval;
        config.enabled = true;
        self.worker_recovery_config = Some(config);
        self
    }

    /// Sets the inactive worker sweep interval (`sweepInterval`).
    pub fn sweep_interval(mut self, interval: Duration) -> Self {
        let mut config = self.worker_recovery_config.unwrap_or_default();
        config.sweep_interval = interval;
        config.enabled = true;
        self.worker_recovery_config = Some(config);
        self
    }

    /// Sets the inactive worker threshold (`sweepThreshold`).
    pub fn sweep_threshold(mut self, threshold: Duration) -> Self {
        let mut config = self.worker_recovery_config.unwrap_or_default();
        config.sweep_threshold = threshold;
        config.enabled = true;
        self.worker_recovery_config = Some(config);
        self
    }

    /// Sets the delay before jobs recovered from dead workers become eligible again.
    pub fn recovery_delay(mut self, delay: Duration) -> Self {
        let mut config = self.worker_recovery_config.unwrap_or_default();
        config.recovery_delay = delay;
        config.enabled = true;
        self.worker_recovery_config = Some(config);
        self
    }
}
