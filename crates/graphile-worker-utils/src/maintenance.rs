use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, DbParams};
use graphile_worker_migrations::{migrate as run_migrations, MigrateError};
use indoc::formatdoc;

use super::client::WorkerUtils;
use super::types::CleanupTask;
use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_queries::schema_names::PrivateTable;
use graphile_worker_queries::task_identifiers::get_tasks_details;
use graphile_worker_queries::worker_heartbeat::active::list_active_workers as list_heartbeat_workers;
use graphile_worker_recovery::{
    sweep_stale_workers as run_recovery_sweep, ActiveWorkerRow, WorkerRecoveryConfig,
};
use graphile_worker_recovery::{SweepStaleWorkersOptions, SweepStaleWorkersResult};

pub(super) async fn list_active_workers(
    utils: &WorkerUtils,
    executor: impl DbExecutorArg,
    sweep_threshold: Duration,
) -> Result<Vec<ActiveWorkerRow>, GraphileWorkerError> {
    list_heartbeat_workers(executor, &utils.schema, sweep_threshold).await
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
    for task in tasks {
        if !matches!(task, CleanupTask::GcTaskIdentifiers) {
            execute_cleanup_task(utils, task, &[]).await?;
            continue;
        }

        // Queue and task GC may wait for a caller-owned add transaction. Never
        // hold the task cache across that wait: the transaction may need it for
        // another add. It is taken only to read the keep list and to store the
        // refreshed details.
        let task_names = utils.task_details.task_names().await;
        execute_cleanup_task(utils, task, &task_names).await?;
        refresh_task_details(utils).await?;
    }

    Ok(())
}

async fn refresh_task_details(utils: &WorkerUtils) -> Result<(), GraphileWorkerError> {
    loop {
        let snapshot = utils.task_details.read().await.clone();
        let refreshed =
            get_tasks_details(&utils.database, &utils.schema, snapshot.task_names()).await?;
        let mut guard = utils.task_details.write().await;
        if *guard != snapshot {
            // A registration changed the cache during database I/O. Retry from
            // its latest contents rather than replacing it with stale details.
            continue;
        }
        *guard = refreshed;
        return Ok(());
    }
}

async fn execute_cleanup_task(
    utils: &WorkerUtils,
    task: &CleanupTask,
    task_names: &[String],
) -> Result<(), GraphileWorkerError> {
    let table = match task {
        CleanupTask::GcJobQueues => PrivateTable::JobQueues,
        CleanupTask::GcTaskIdentifiers => PrivateTable::Tasks,
        _ => {
            return task
                .execute(&utils.database, &utils.schema, task_names)
                .await;
        }
    };

    let transaction = utils.database.begin().await?;
    let mut executor = &transaction;
    // The DELETE must see additions committed while we waited for the lock,
    // even when the database's default transaction isolation is stricter.
    executor
        .execute(
            "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
            DbParams::new(),
        )
        .await?;
    // A queued table lock also blocks later statements that write the table.
    // Bound that wait, including when this future is dropped before LOCK
    // completes.
    executor
        .execute("SET LOCAL lock_timeout = '1s'", DbParams::new())
        .await?;
    let table = table.qualified(&utils.schema);
    // add_jobs takes ROW EXCLUSIVE on the queue and task tables even when its
    // INSERT finds an existing row. Hold the conflicting lock through deletion,
    // and take the DELETE's snapshot only after every earlier add commits.
    let lock = formatdoc!("LOCK TABLE {table} IN SHARE ROW EXCLUSIVE MODE");
    executor.execute(&lock, DbParams::new()).await?;
    task.execute(&transaction, &utils.schema, task_names)
        .await?;
    transaction.commit().await?;
    Ok(())
}

pub(super) async fn migrate(utils: &WorkerUtils) -> Result<(), MigrateError> {
    run_migrations(&utils.database, &utils.schema).await?;
    Ok(())
}
