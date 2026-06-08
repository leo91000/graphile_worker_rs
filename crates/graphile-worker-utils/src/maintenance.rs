use std::time::Duration;

use graphile_worker_migrations::{migrate as run_migrations, MigrateError};

use super::client::WorkerUtils;
use super::types::CleanupTask;
use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_queries::task_identifiers::get_tasks_details;
use graphile_worker_queries::worker_heartbeat::active::list_active_workers as list_heartbeat_workers;
use graphile_worker_recovery::{
    sweep_stale_workers as run_recovery_sweep, ActiveWorkerRow, WorkerRecoveryConfig,
};
use graphile_worker_recovery::{SweepStaleWorkersOptions, SweepStaleWorkersResult};

pub(super) async fn list_active_workers(
    utils: &WorkerUtils,
    sweep_threshold: Duration,
) -> Result<Vec<ActiveWorkerRow>, GraphileWorkerError> {
    list_heartbeat_workers(&utils.database, &utils.schema, sweep_threshold).await
}

pub(super) async fn sweep_stale_workers(
    utils: &WorkerUtils,
    options: SweepStaleWorkersOptions,
) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
    let recovery_config = WorkerRecoveryConfig::default();
    sweep_stale_workers_with_config(utils, &recovery_config, options).await
}

pub(super) async fn sweep_stale_workers_with_config(
    utils: &WorkerUtils,
    recovery_config: &WorkerRecoveryConfig,
    options: SweepStaleWorkersOptions,
) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
    run_recovery_sweep(
        &utils.database,
        &utils.schema,
        utils.hooks.as_ref(),
        "worker_utils",
        recovery_config,
        options,
    )
    .await
}

pub(super) async fn cleanup(
    utils: &WorkerUtils,
    tasks: &[CleanupTask],
) -> Result<(), GraphileWorkerError> {
    let should_refresh_task_identifiers = tasks
        .iter()
        .any(|task| matches!(task, CleanupTask::GcTaskIdentifiers));

    if should_refresh_task_identifiers {
        let mut guard = utils.task_details.write().await;
        let task_names = guard.task_names();

        for task in tasks {
            task.execute(&utils.database, &utils.schema, &task_names)
                .await?;
        }

        let refreshed = get_tasks_details(&utils.database, &utils.schema, task_names).await?;
        *guard = refreshed;
        return Ok(());
    }

    for task in tasks {
        task.execute(&utils.database, &utils.schema, &[]).await?;
    }

    Ok(())
}

pub(super) async fn migrate(utils: &WorkerUtils) -> Result<(), MigrateError> {
    run_migrations(&utils.database, &utils.schema).await?;
    Ok(())
}
