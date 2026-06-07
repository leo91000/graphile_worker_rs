use std::sync::Arc;

use futures::FutureExt;
use tracing::{debug, error, warn};

use crate::background_tasks::BackgroundTasks;
use crate::recovery::{sweep_stale_workers, SweepStaleWorkersOptions};
use crate::sql::worker_heartbeat::registration::{worker_deregister, worker_heartbeat};
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
        &worker.schema,
        &worker.worker_id,
        metadata,
    )
    .await
}

pub(crate) async fn deregister_worker(
    worker: &Worker,
) -> Result<(), crate::errors::GraphileWorkerError> {
    if !worker.recovery_config.enabled {
        return Ok(());
    }

    worker_deregister(&worker.database, &worker.schema, &worker.worker_id).await
}

pub(crate) struct RecoveryTasks {
    tasks: BackgroundTasks,
}

impl RecoveryTasks {
    pub(crate) fn empty() -> Self {
        Self {
            tasks: BackgroundTasks::new("recovery"),
        }
    }

    pub(crate) async fn stop(self) {
        self.tasks.stop().await;
    }
}

pub(crate) fn spawn_recovery_tasks(worker: &Worker) -> RecoveryTasks {
    if !worker.recovery_config.enabled {
        return RecoveryTasks::empty();
    }

    let worker = Arc::new(worker.clone_for_recovery());
    let heartbeat_worker = worker.clone();
    let heartbeat_handle = runtime::spawn(async move {
        if let Err(error) = run_heartbeat_loop(heartbeat_worker).await {
            error!(error = %error, "Worker heartbeat loop failed");
        }
    });

    let sweep_worker = worker;
    let sweep_handle = runtime::spawn(async move {
        if let Err(error) = run_sweeper_loop(sweep_worker).await {
            error!(error = %error, "Worker recovery sweeper failed");
        }
    });

    let mut tasks = BackgroundTasks::new("recovery");
    tasks.push(heartbeat_handle);
    tasks.push(sweep_handle);

    RecoveryTasks { tasks }
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
                    &worker.schema,
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
    let result = sweep_stale_workers(
        &worker.database,
        &worker.schema,
        Some(&worker.hooks),
        &worker.worker_id,
        &worker.recovery_config,
        SweepStaleWorkersOptions {
            ..Default::default()
        },
    )
    .await?;

    if result.worker_ids.is_empty() {
        debug!("No stale workers found during recovery sweep");
    }

    Ok(())
}

fn worker_recovery_metadata() -> Option<serde_json::Value> {
    Some(serde_json::json!({
        "pid": std::process::id(),
    }))
}
