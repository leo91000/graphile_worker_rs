use std::sync::Arc;

use graphile_worker_database::{Database, Schema};
use graphile_worker_lifecycle_hooks::{HookRegistry, WorkerRecoveredContext};
use tracing::{debug, warn};

use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_queries::worker_heartbeat::lock::try_acquire_sweep_lock;
use graphile_worker_queries::worker_heartbeat::stale::delete_stale_workers;

use super::config::WorkerRecoveryConfig;
use super::types::{SweepStaleWorkersOptions, SweepStaleWorkersResult};

mod discovery;
mod jobs;

use discovery::find_dead_worker_ids;
use jobs::recover_jobs_from_workers;

pub async fn sweep_stale_workers(
    database: &Database,
    schema: &Schema,
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

    let dead_worker_ids = find_dead_worker_ids(&transaction, schema, &config).await?;

    if config.dry_run {
        transaction.commit().await?;
        return Ok(SweepStaleWorkersResult {
            worker_ids: dead_worker_ids,
            recovered_count: 0,
        });
    }

    let recovered_count = recover_jobs_from_workers(
        &transaction,
        schema,
        hooks,
        worker_id,
        &dead_worker_ids,
        config.recovery_delay,
    )
    .await?;

    delete_stale_workers(&transaction, schema, &dead_worker_ids).await?;
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
