use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use graphile_worker_database::{Database, DbExecutorArg};
use graphile_worker_lifecycle_hooks::{
    FailureReason, HookRegistry, JobInterruptedContext, WorkerRecoveredContext,
};
use tracing::{debug, warn};

use crate::errors::GraphileWorkerError;
use crate::sql::recover_workers::{get_locked_jobs_for_recovery, recover_dead_worker_jobs};
use crate::sql::worker_heartbeat::{
    delete_stale_workers, get_worker_last_heartbeat, list_orphan_locked_workers,
    list_stale_workers, try_acquire_sweep_lock, worker_holds_resilient_locks,
};

use super::config::WorkerRecoveryConfig;
use super::job_recovery::apply_job_recovery;
use super::types::{
    JobRecoveryRequest, ResolvedSweepConfig, SweepStaleWorkersOptions, SweepStaleWorkersResult,
};

pub(crate) async fn sweep_stale_workers(
    database: &Database,
    escaped_schema: &str,
    hooks: Option<&Arc<HookRegistry>>,
    worker_id: &str,
    defaults: &WorkerRecoveryConfig,
    options: SweepStaleWorkersOptions,
) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
    let config = options.resolve(defaults);

    let transaction = database.begin().await?;
    if !try_acquire_sweep_lock(&transaction).await? {
        debug!("Another worker is already running recovery sweep");
        return Ok(SweepStaleWorkersResult {
            worker_ids: Vec::new(),
            recovered_count: 0,
        });
    }

    let dead_worker_ids = find_dead_worker_ids(&transaction, escaped_schema, &config).await?;

    if config.dry_run {
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
        config.recovery_delay,
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
    config: &ResolvedSweepConfig,
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
    config: &ResolvedSweepConfig,
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

    let threshold = config.effective_sweep_threshold(true);
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

        let outcome = apply_job_recovery(
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

        if outcome.was_handled() {
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
