use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{FailureReason, HookRegistry};
use serde::Serialize;

use super::config::WorkerRecoveryConfig;

#[derive(Debug, Clone, Default)]
pub struct SweepStaleWorkersOptions {
    pub sweep_threshold: Option<Duration>,
    pub recovery_delay: Option<Duration>,
    pub dry_run: bool,
}

impl SweepStaleWorkersOptions {
    pub fn resolve(&self, defaults: &WorkerRecoveryConfig) -> ResolvedSweepConfig {
        ResolvedSweepConfig {
            sweep_threshold: self.sweep_threshold.unwrap_or(defaults.sweep_threshold),
            recovery_delay: self.recovery_delay.unwrap_or(defaults.recovery_delay),
            resilient_sweep_threshold_multiplier: defaults.resilient_sweep_threshold_multiplier,
            resilient_job_flags: defaults.resilient_job_flags.clone(),
            dry_run: self.dry_run,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSweepConfig {
    pub sweep_threshold: Duration,
    pub recovery_delay: Duration,
    pub resilient_sweep_threshold_multiplier: u32,
    pub resilient_job_flags: Vec<String>,
    pub dry_run: bool,
}

impl ResolvedSweepConfig {
    pub(crate) fn effective_sweep_threshold(&self, has_resilient_lock: bool) -> Duration {
        if has_resilient_lock {
            self.sweep_threshold * self.resilient_sweep_threshold_multiplier
        } else {
            self.sweep_threshold
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SweepStaleWorkersResult {
    pub worker_ids: Vec<String>,
    pub recovered_count: i32,
}

pub(crate) struct JobRecoveryRequest<'a> {
    pub hooks: Option<&'a Arc<HookRegistry>>,
    pub worker_id: &'a str,
    pub job: Arc<Job>,
    pub previous_worker_id: &'a str,
    pub reason: FailureReason,
    pub recovery_delay: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobRecoveryOutcome {
    Recovered,
    FailedWithBackoff,
    Skipped,
}

impl JobRecoveryOutcome {
    pub(crate) fn was_handled(self) -> bool {
        !matches!(self, Self::Skipped)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    pub last_heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub is_stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_options_resolve_against_recovery_defaults() {
        let defaults = WorkerRecoveryConfig::default()
            .sweep_threshold(Duration::from_secs(11))
            .recovery_delay(Duration::from_secs(22))
            .resilient_sweep_threshold_multiplier(4)
            .resilient_job_flags(vec!["custom".to_string()]);

        let resolved = SweepStaleWorkersOptions {
            recovery_delay: Some(Duration::from_secs(33)),
            dry_run: true,
            ..Default::default()
        }
        .resolve(&defaults);

        assert_eq!(resolved.sweep_threshold, Duration::from_secs(11));
        assert_eq!(resolved.recovery_delay, Duration::from_secs(33));
        assert_eq!(resolved.resilient_sweep_threshold_multiplier, 4);
        assert_eq!(resolved.resilient_job_flags, vec!["custom"]);
        assert!(resolved.dry_run);
    }

    #[test]
    fn recovery_outcomes_distinguish_handled_from_skipped() {
        assert!(JobRecoveryOutcome::Recovered.was_handled());
        assert!(JobRecoveryOutcome::FailedWithBackoff.was_handled());
        assert!(!JobRecoveryOutcome::Skipped.was_handled());
    }
}
