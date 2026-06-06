use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use graphile_worker_database::{Database, DbExecutorArg, DbParams, DbValue};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    FailureReason, HookRegistry, JobInterruptedContext, JobRecoveryContext, JobRecoveryResult,
    WorkerRecoveredContext,
};
use indoc::formatdoc;
use serde::Serialize;
use tracing::{debug, warn};

use crate::errors::GraphileWorkerError;
use crate::sql::fail_job::fail_job;
use crate::sql::recover_workers::{get_locked_jobs_for_recovery, recover_dead_worker_jobs};
use crate::sql::return_jobs::return_job_for_recovery;
use crate::sql::worker_heartbeat::{
    delete_stale_workers, get_worker_last_heartbeat, list_orphan_locked_workers,
    list_stale_workers, try_acquire_sweep_lock, worker_holds_resilient_locks,
};

const RECOVERY_LAST_ERROR: &str = "Job recovered after worker interruption";

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

    pub fn shutdown_grace_period(mut self, value: Duration) -> Self {
        self.shutdown_grace_period = value;
        self.enabled = true;
        self
    }

    pub fn shutdown_recovery_delay(mut self, value: Duration) -> Self {
        self.shutdown_recovery_delay = value;
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

#[derive(Debug, Clone, Default)]
pub struct SweepStaleWorkersOptions {
    pub sweep_threshold: Option<Duration>,
    pub recovery_delay: Option<Duration>,
    pub recovery_config: Option<WorkerRecoveryConfig>,
    pub dry_run: bool,
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

pub(crate) async fn apply_job_recovery(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    request: JobRecoveryRequest<'_>,
) -> Result<bool, GraphileWorkerError> {
    let action = match request.hooks {
        Some(hooks) if !hooks.is_empty() => {
            hooks
                .intercept(JobRecoveryContext {
                    job: request.job.clone(),
                    worker_id: request.worker_id.to_string(),
                    previous_worker_id: request.previous_worker_id.to_string(),
                    reason: request.reason,
                })
                .await
        }
        _ => JobRecoveryResult::Default,
    };

    match action {
        JobRecoveryResult::Default => {
            return_job_for_recovery(
                &mut executor,
                &request.job,
                escaped_schema,
                request.previous_worker_id,
                Some(request.recovery_delay),
                Some(RECOVERY_LAST_ERROR),
            )
            .await?;
            Ok(true)
        }
        JobRecoveryResult::Reschedule { run_at, attempts } => {
            return_job_for_recovery(
                &mut executor,
                &request.job,
                escaped_schema,
                request.previous_worker_id,
                None,
                Some(RECOVERY_LAST_ERROR),
            )
            .await?;
            set_recovered_job_schedule(
                &mut executor,
                escaped_schema,
                *request.job.id(),
                run_at,
                attempts,
            )
            .await?;
            Ok(true)
        }
        JobRecoveryResult::FailWithBackoff => {
            fail_job(
                &mut executor,
                &request.job,
                escaped_schema,
                request.previous_worker_id,
                &format!("{:?}", request.reason),
                None,
            )
            .await?;
            Ok(true)
        }
        JobRecoveryResult::Skip => Ok(false),
    }
}

async fn set_recovered_job_schedule(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    job_id: i64,
    run_at: DateTime<Utc>,
    attempts: Option<i16>,
) -> Result<(), GraphileWorkerError> {
    let sql = formatdoc!(
        r#"
            UPDATE {escaped_schema}._private_jobs AS jobs
            SET
                run_at = $2::timestamptz,
                attempts = COALESCE($3::int, jobs.attempts),
                updated_at = now()
            WHERE id = $1::bigint;
        "#
    );

    executor
        .execute(
            &sql,
            DbParams::from(vec![
                DbValue::I64(job_id),
                DbValue::TimestampTz(run_at),
                DbValue::I32Opt(attempts.map(i32::from)),
            ]),
        )
        .await?;

    Ok(())
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

pub(crate) async fn sweep_stale_workers(
    database: &Database,
    escaped_schema: &str,
    hooks: Option<&Arc<HookRegistry>>,
    worker_id: &str,
    options: SweepStaleWorkersOptions,
) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
    let mut config = options.recovery_config.unwrap_or_default();
    if let Some(sweep_threshold) = options.sweep_threshold {
        config.sweep_threshold = sweep_threshold;
    }
    let recovery_delay = options.recovery_delay.unwrap_or(config.recovery_delay);

    let transaction = database.begin().await?;
    if !try_acquire_sweep_lock(&transaction).await? {
        debug!("Another worker is already running recovery sweep");
        return Ok(SweepStaleWorkersResult {
            worker_ids: Vec::new(),
            recovered_count: 0,
        });
    }

    let dead_worker_ids = find_dead_worker_ids(&transaction, escaped_schema, &config).await?;

    if options.dry_run {
        transaction.commit().await?;
        return Ok(SweepStaleWorkersResult {
            worker_ids: dead_worker_ids,
            recovered_count: 0,
        });
    }

    let recovered_count = recover_jobs_from_workers(
        &transaction,
        escaped_schema,
        hooks,
        worker_id,
        &dead_worker_ids,
        recovery_delay,
    )
    .await?;

    delete_stale_workers(&transaction, escaped_schema, &dead_worker_ids).await?;
    transaction.commit().await?;

    if let Some(hooks) = hooks.filter(|hooks| !hooks.is_empty()) {
        hooks
            .emit(WorkerRecoveredContext {
                worker_id: worker_id.to_string(),
                dead_worker_ids: dead_worker_ids.clone(),
                jobs_recovered: recovered_count.max(0) as usize,
            })
            .await;
    }

    if !dead_worker_ids.is_empty() {
        warn!(
            dead_worker_ids = ?dead_worker_ids,
            recovered_count,
            "Recovered jobs from inactive workers"
        );
    }

    Ok(SweepStaleWorkersResult {
        worker_ids: dead_worker_ids,
        recovered_count,
    })
}

async fn find_dead_worker_ids(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    config: &WorkerRecoveryConfig,
) -> Result<Vec<String>, GraphileWorkerError> {
    let stale_worker_ids =
        list_stale_workers(&mut executor, escaped_schema, config.sweep_threshold).await?;

    let mut dead_worker_ids = Vec::new();
    for worker_id in stale_worker_ids {
        if should_recover_worker(&mut executor, escaped_schema, &worker_id, config).await? {
            dead_worker_ids.push(worker_id);
        }
    }

    let orphan_worker_ids =
        list_orphan_locked_workers(&mut executor, escaped_schema, config.sweep_threshold).await?;

    for worker_id in orphan_worker_ids {
        if !dead_worker_ids.iter().any(|id| id == &worker_id) {
            dead_worker_ids.push(worker_id);
        }
    }

    Ok(dead_worker_ids)
}

async fn should_recover_worker(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_id: &str,
    config: &WorkerRecoveryConfig,
) -> Result<bool, GraphileWorkerError> {
    let has_resilient_locks = worker_holds_resilient_locks(
        &mut executor,
        escaped_schema,
        worker_id,
        &config.resilient_job_flags,
    )
    .await?;

    if !has_resilient_locks {
        return Ok(true);
    }

    let Some(last_heartbeat) =
        get_worker_last_heartbeat(&mut executor, escaped_schema, worker_id).await?
    else {
        return Ok(true);
    };

    let threshold = effective_sweep_threshold(config, true);
    let elapsed = Utc::now()
        .signed_duration_since(last_heartbeat)
        .to_std()
        .unwrap_or_default();

    Ok(elapsed >= threshold)
}

async fn recover_jobs_from_workers(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    hooks: Option<&Arc<HookRegistry>>,
    worker_id: &str,
    worker_ids: &[String],
    recovery_delay: Duration,
) -> Result<i32, GraphileWorkerError> {
    if worker_ids.is_empty() {
        return Ok(0);
    }

    let has_hooks = hooks.is_some_and(|hooks| !hooks.is_empty());
    if !has_hooks {
        return recover_dead_worker_jobs(&mut executor, escaped_schema, worker_ids, recovery_delay)
            .await;
    }

    let jobs = get_locked_jobs_for_recovery(&mut executor, escaped_schema, worker_ids).await?;
    let mut recovered_count = 0;

    for job in jobs {
        let Some(previous_worker_id) = job.locked_by().as_deref() else {
            continue;
        };

        let recovered = apply_job_recovery(
            &mut executor,
            escaped_schema,
            JobRecoveryRequest {
                hooks,
                worker_id,
                job: job.clone(),
                previous_worker_id,
                reason: FailureReason::WorkerCrashed,
                recovery_delay,
            },
        )
        .await?;

        if recovered {
            recovered_count += 1;
            if let Some(hooks) = hooks {
                hooks
                    .emit(JobInterruptedContext {
                        job,
                        worker_id: worker_id.to_string(),
                        reason: FailureReason::WorkerCrashed,
                    })
                    .await;
            }
        }
    }

    Ok(recovered_count)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    pub last_heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub is_stale: bool,
}
