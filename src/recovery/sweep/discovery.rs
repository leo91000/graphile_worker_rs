use chrono::Utc;
use graphile_worker_database::{DbExecutorArg, Schema};

use crate::errors::GraphileWorkerError;
use graphile_worker_queries::worker_heartbeat::stale::{
    get_worker_last_heartbeat, list_orphan_locked_workers, list_stale_workers,
    worker_holds_resilient_locks,
};

use super::super::types::ResolvedSweepConfig;

pub(super) async fn find_dead_worker_ids(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    config: &ResolvedSweepConfig,
) -> Result<Vec<String>, GraphileWorkerError> {
    let stale_worker_ids =
        list_stale_workers(&mut executor, schema, config.sweep_threshold).await?;

    let mut dead_worker_ids = Vec::new();
    for worker_id in stale_worker_ids {
        if should_recover_worker(&mut executor, schema, &worker_id, config).await? {
            dead_worker_ids.push(worker_id);
        }
    }

    let orphan_worker_ids =
        list_orphan_locked_workers(&mut executor, schema, config.sweep_threshold).await?;

    for worker_id in orphan_worker_ids {
        if !dead_worker_ids.iter().any(|id| id == &worker_id) {
            dead_worker_ids.push(worker_id);
        }
    }

    Ok(dead_worker_ids)
}

async fn should_recover_worker(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    worker_id: &str,
    config: &ResolvedSweepConfig,
) -> Result<bool, GraphileWorkerError> {
    let has_resilient_locks = worker_holds_resilient_locks(
        &mut executor,
        schema,
        worker_id,
        &config.resilient_job_flags,
    )
    .await?;

    if !has_resilient_locks {
        return Ok(true);
    }

    let Some(last_heartbeat) = get_worker_last_heartbeat(&mut executor, schema, worker_id).await?
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
