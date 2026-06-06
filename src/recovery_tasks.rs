use std::sync::Arc;

use chrono::Utc;
use futures::FutureExt;
use graphile_worker_lifecycle_hooks::WorkerRecoveredContext;
use tracing::{debug, error, warn};

use crate::recovery::{effective_sweep_threshold, WorkerRecoveryConfig};
use crate::sql::recover_workers::recover_dead_worker_jobs;
use crate::sql::worker_heartbeat::{
    delete_stale_workers, get_worker_last_heartbeat, list_orphan_locked_workers,
    list_stale_workers, release_sweep_lock, try_acquire_sweep_lock, worker_deregister,
    worker_heartbeat, worker_holds_resilient_locks,
};
use crate::Worker;
use graphile_worker_runtime as runtime;

pub(crate) async fn register_worker(
    worker: &Worker,
    metadata: Option<serde_json::Value>,
) -> Result<(), crate::errors::GraphileWorkerError> {
    if !worker.recovery_config.enabled {
        return Ok(());
    }

    worker_heartbeat(
        &worker.database,
        &worker.escaped_schema,
        &worker.worker_id,
        metadata,
    )
    .await
}

pub(crate) async fn deregister_worker(worker: &Worker) -> Result<(), crate::errors::GraphileWorkerError> {
    if !worker.recovery_config.enabled {
        return Ok(());
    }

    worker_deregister(&worker.database, &worker.escaped_schema, &worker.worker_id).await
}

pub(crate) fn spawn_recovery_tasks(worker: Arc<Worker>) {
    if !worker.recovery_config.enabled {
        return;
    }

    let heartbeat_worker = worker.clone();
    runtime::spawn(async move {
        if let Err(error) = run_heartbeat_loop(heartbeat_worker).await {
            error!(error = %error, "Worker heartbeat loop failed");
        }
    });

    let sweep_worker = worker;
    runtime::spawn(async move {
        if let Err(error) = run_sweeper_loop(sweep_worker).await {
            error!(error = %error, "Worker recovery sweeper failed");
        }
    });
}

async fn run_heartbeat_loop(worker: Arc<Worker>) -> Result<(), crate::errors::GraphileWorkerError> {
    let mut interval = runtime::interval(worker.recovery_config.heartbeat_interval);
    let mut shutdown_signal = worker.shutdown_signal.clone();

    loop {
        futures::select_biased! {
            _ = (&mut shutdown_signal).fuse() => break,
            _ = interval.tick().fuse() => {
                worker_heartbeat(
                    &worker.database,
                    &worker.escaped_schema,
                    &worker.worker_id,
                    worker_recovery_metadata(),
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn run_sweeper_loop(worker: Arc<Worker>) -> Result<(), crate::errors::GraphileWorkerError> {
    let mut interval = runtime::interval(worker.recovery_config.sweep_interval);
    let mut shutdown_signal = worker.shutdown_signal.clone();

    loop {
        futures::select_biased! {
            _ = (&mut shutdown_signal).fuse() => break,
            _ = interval.tick().fuse() => {
                if let Err(error) = sweep_once(worker.clone()).await {
                    warn!(error = %error, "Worker recovery sweep failed");
                }
            }
        }
    }

    Ok(())
}

async fn sweep_once(worker: Arc<Worker>) -> Result<(), crate::errors::GraphileWorkerError> {
    if !try_acquire_sweep_lock(&worker.database).await? {
        debug!("Another worker is already running recovery sweep");
        return Ok(());
    }

    let sweep_result = perform_sweep(&worker).await;
    if let Err(error) = release_sweep_lock(&worker.database).await {
        warn!(error = %error, "Failed to release recovery sweep advisory lock");
    }
    sweep_result
}

async fn perform_sweep(worker: &Worker) -> Result<(), crate::errors::GraphileWorkerError> {
    let config = &worker.recovery_config;
    let stale_worker_ids =
        list_stale_workers(&worker.database, &worker.escaped_schema, config.sweep_threshold).await?;

    let mut dead_worker_ids = Vec::new();
    for worker_id in stale_worker_ids {
        if should_recover_worker(worker, &worker_id, config).await? {
            dead_worker_ids.push(worker_id);
        }
    }

    let orphan_worker_ids = list_orphan_locked_workers(
        &worker.database,
        &worker.escaped_schema,
        config.sweep_threshold,
    )
    .await?;

    for worker_id in orphan_worker_ids {
        if !dead_worker_ids.iter().any(|id| id == &worker_id) {
            dead_worker_ids.push(worker_id);
        }
    }

    if dead_worker_ids.is_empty() {
        return Ok(());
    }

    let recovered_count = recover_dead_worker_jobs(
        &worker.database,
        &worker.escaped_schema,
        &dead_worker_ids,
        config.recovery_delay,
    )
    .await?;

    delete_stale_workers(&worker.database, &worker.escaped_schema, &dead_worker_ids).await?;

    if !worker.hooks.is_empty() {
        worker
            .hooks
            .emit(WorkerRecoveredContext {
                worker_id: worker.worker_id.clone(),
                dead_worker_ids: dead_worker_ids.clone(),
                jobs_recovered: recovered_count.max(0) as usize,
            })
            .await;
    }

    warn!(
        dead_worker_ids = ?dead_worker_ids,
        recovered_count,
        "Recovered jobs from inactive workers"
    );

    Ok(())
}

async fn should_recover_worker(
    worker: &Worker,
    worker_id: &str,
    config: &WorkerRecoveryConfig,
) -> Result<bool, crate::errors::GraphileWorkerError> {
    let has_resilient_locks = worker_holds_resilient_locks(
        &worker.database,
        &worker.escaped_schema,
        worker_id,
        &config.resilient_job_flags,
    )
    .await?;

    if !has_resilient_locks {
        return Ok(true);
    }

    let Some(last_heartbeat) = get_worker_last_heartbeat(
        &worker.database,
        &worker.escaped_schema,
        worker_id,
    )
    .await?
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

fn worker_recovery_metadata() -> Option<serde_json::Value> {
    Some(serde_json::json!({
        "pid": std::process::id(),
    }))
}