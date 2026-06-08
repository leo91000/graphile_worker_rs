use std::time::Duration;

use graphile_worker_job::Job;

/// Job flag indicating the job may run for a long time and should use a
/// longer sweep threshold before being recovered from a seemingly-dead worker.
pub const INFRASTRUCTURE_RESILIENT_FLAG: &str = "infrastructure_resilient";

/// Dead worker recovery configuration.
///
/// Timing options align with [Graphile Worker Pro](https://worker.graphile.org/docs/pro/recovery):
/// - `heartbeat_interval` — `heartbeatInterval`
/// - `sweep_interval` — `sweepInterval`
/// - `sweep_threshold` — `sweepThreshold`
#[derive(Debug, Clone)]
pub struct WorkerRecoveryConfig {
    /// Worker check-in frequency (`heartbeatInterval`).
    pub heartbeat_interval: Duration,
    /// Inactive worker check frequency (`sweepInterval`).
    pub sweep_interval: Duration,
    /// Time since last heartbeat before a worker is deemed inactive (`sweepThreshold`).
    pub sweep_threshold: Duration,
    /// Delay before jobs recovered from dead workers are eligible to run again.
    pub recovery_delay: Duration,
    /// Multiplier applied to `sweep_threshold` when a worker holds jobs with resilient flags.
    pub resilient_sweep_threshold_multiplier: u32,
    /// Job flags that trigger the extended sweep threshold.
    pub resilient_job_flags: Vec<String>,
    /// Whether heartbeat registration and sweeping are enabled.
    pub enabled: bool,
}

impl Default for WorkerRecoveryConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(30),
            sweep_interval: Duration::from_secs(60),
            sweep_threshold: Duration::from_secs(5 * 60),
            recovery_delay: Duration::from_secs(30),
            resilient_sweep_threshold_multiplier: 3,
            resilient_job_flags: vec![INFRASTRUCTURE_RESILIENT_FLAG.to_string()],
            enabled: false,
        }
    }
}

impl WorkerRecoveryConfig {
    pub fn heartbeat_interval(mut self, value: Duration) -> Self {
        self.heartbeat_interval = value;
        self.enabled = true;
        self
    }

    pub fn sweep_interval(mut self, value: Duration) -> Self {
        self.sweep_interval = value;
        self.enabled = true;
        self
    }

    pub fn sweep_threshold(mut self, value: Duration) -> Self {
        self.sweep_threshold = value;
        self.enabled = true;
        self
    }

    pub fn recovery_delay(mut self, value: Duration) -> Self {
        self.recovery_delay = value;
        self.enabled = true;
        self
    }

    pub fn resilient_sweep_threshold_multiplier(mut self, value: u32) -> Self {
        self.resilient_sweep_threshold_multiplier = value;
        self.enabled = true;
        self
    }

    pub fn resilient_job_flags(mut self, flags: Vec<String>) -> Self {
        self.resilient_job_flags = flags;
        self.enabled = true;
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}

pub fn job_has_resilient_flag(job: &Job, config: &WorkerRecoveryConfig) -> bool {
    let Some(flags) = job.flags() else {
        return false;
    };
    config.resilient_job_flags.iter().any(|flag| {
        flags
            .get(flag)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    })
}
