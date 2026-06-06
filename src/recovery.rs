use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use graphile_worker_database::Database;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    FailureReason, HookRegistry, JobRecoveryContext, JobRecoveryResult,
};

use crate::errors::GraphileWorkerError;
use crate::sql::fail_job::fail_job;
use crate::sql::return_jobs::return_job_for_recovery;
use crate::worker_utils::{RescheduleJobOptions, WorkerUtils};

/// Job flag indicating the job may run for a long time and should use a
/// longer sweep threshold before being recovered from a seemingly-dead worker.
pub const INFRASTRUCTURE_RESILIENT_FLAG: &str = "infrastructure_resilient";

/// Worker crash/shutdown recovery configuration.
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
    /// Delay before recovered jobs are eligible to run again.
    pub recovery_delay: Duration,
    /// Time to let in-flight tasks finish after a shutdown signal.
    pub shutdown_grace_period: Duration,
    /// Delay before shutdown-aborted jobs are retried.
    pub shutdown_recovery_delay: Duration,
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
            shutdown_grace_period: Duration::from_secs(5),
            shutdown_recovery_delay: Duration::from_secs(30),
            resilient_sweep_threshold_multiplier: 3,
            resilient_job_flags: vec![INFRASTRUCTURE_RESILIENT_FLAG.to_string()],
            enabled: true,
        }
    }
}

impl WorkerRecoveryConfig {
    pub fn heartbeat_interval(mut self, value: Duration) -> Self {
        self.heartbeat_interval = value;
        self
    }

    pub fn sweep_interval(mut self, value: Duration) -> Self {
        self.sweep_interval = value;
        self
    }

    pub fn sweep_threshold(mut self, value: Duration) -> Self {
        self.sweep_threshold = value;
        self
    }

    pub fn recovery_delay(mut self, value: Duration) -> Self {
        self.recovery_delay = value;
        self
    }

    pub fn shutdown_grace_period(mut self, value: Duration) -> Self {
        self.shutdown_grace_period = value;
        self
    }

    pub fn shutdown_recovery_delay(mut self, value: Duration) -> Self {
        self.shutdown_recovery_delay = value;
        self
    }

    pub fn resilient_sweep_threshold_multiplier(mut self, value: u32) -> Self {
        self.resilient_sweep_threshold_multiplier = value;
        self
    }

    pub fn resilient_job_flags(mut self, flags: Vec<String>) -> Self {
        self.resilient_job_flags = flags;
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

pub(crate) async fn apply_job_recovery(
    database: &Database,
    escaped_schema: &str,
    hooks: &Arc<HookRegistry>,
    worker_id: &str,
    job: Arc<Job>,
    previous_worker_id: &str,
    reason: FailureReason,
    recovery_delay: Duration,
) -> Result<(), GraphileWorkerError> {
    let action = if hooks.is_empty() {
        JobRecoveryResult::Default
    } else {
        hooks
            .intercept(JobRecoveryContext {
                job: job.clone(),
                worker_id: worker_id.to_string(),
                previous_worker_id: previous_worker_id.to_string(),
                reason,
            })
            .await
    };

    match action {
        JobRecoveryResult::Default => {
            return_job_for_recovery(
                database,
                &job,
                escaped_schema,
                previous_worker_id,
                Some(recovery_delay),
                Some("Job recovered after worker interruption"),
            )
            .await
        }
        JobRecoveryResult::Reschedule { run_at, attempts } => {
            return_job_for_recovery(
                database,
                &job,
                escaped_schema,
                previous_worker_id,
                None,
                Some("Job recovered after worker interruption"),
            )
            .await?;
            let utils = WorkerUtils::new(database.clone(), escaped_schema.to_string());
            utils
                .reschedule_jobs(
                    &[*job.id()],
                    RescheduleJobOptions {
                        run_at: Some(run_at),
                        attempts,
                        ..Default::default()
                    },
                )
                .await?;
            Ok(())
        }
        JobRecoveryResult::FailWithBackoff => {
            fail_job(
                database,
                &job,
                escaped_schema,
                previous_worker_id,
                &format!("{reason:?}"),
                None,
            )
            .await
        }
        JobRecoveryResult::Skip => Ok(()),
    }
}

pub(crate) fn effective_sweep_threshold(
    config: &WorkerRecoveryConfig,
    has_resilient_lock: bool,
) -> Duration {
    if has_resilient_lock {
        config.sweep_threshold * config.resilient_sweep_threshold_multiplier
    } else {
        config.sweep_threshold
    }
}

#[derive(Debug, Clone)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    pub last_heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub is_stale: bool,
}