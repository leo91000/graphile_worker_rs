use std::time::Duration;

use graphile_worker_migrations::MigrateError;

use super::client::WorkerUtils;
use super::types::{CleanupTask, RescheduleJobOptions};
use super::{actions, maintenance};
use graphile_worker_job::DbJob;
use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_recovery::{
    ActiveWorkerRow, SweepStaleWorkersOptions, SweepStaleWorkersResult, WorkerRecoveryConfig,
};

impl WorkerUtils {
    /// Removes a job from the queue by its job key.
    ///
    /// Useful for cancelling scheduled jobs that haven't run yet.
    pub async fn remove_job(&self, job_key: &str) -> Result<(), GraphileWorkerError> {
        actions::remove_job(self, job_key).await
    }

    /// Marks multiple jobs as completed.
    pub async fn complete_jobs(&self, ids: &[i64]) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::complete_jobs(self, ids).await
    }

    /// Marks multiple jobs as permanently failed with a reason.
    pub async fn permanently_fail_jobs(
        &self,
        ids: &[i64],
        reason: &str,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::permanently_fail_jobs(self, ids, reason).await
    }

    /// Reschedules multiple jobs with new parameters.
    ///
    /// This allows changing when jobs will run next, their priority,
    /// and their retry behavior.
    pub async fn reschedule_jobs(
        &self,
        ids: &[i64],
        options: RescheduleJobOptions,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::reschedule_jobs(self, ids, options).await
    }

    /// Lists workers registered in the heartbeat table.
    pub async fn list_active_workers(
        &self,
        sweep_threshold: Duration,
    ) -> Result<Vec<ActiveWorkerRow>, GraphileWorkerError> {
        maintenance::list_active_workers(self, sweep_threshold).await
    }

    /// Sweeps inactive workers and orphan locks, recovering their jobs.
    pub async fn sweep_stale_workers(
        &self,
        options: SweepStaleWorkersOptions,
    ) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
        maintenance::sweep_stale_workers(self, options).await
    }

    /// Sweeps inactive workers with an explicit recovery configuration.
    pub async fn sweep_stale_workers_with_config(
        &self,
        recovery_config: &WorkerRecoveryConfig,
        options: SweepStaleWorkersOptions,
    ) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
        maintenance::sweep_stale_workers_with_config(self, recovery_config, options).await
    }

    /// Force unlocks worker records in the database.
    ///
    /// Useful for recovering from situations where workers crashed without
    /// properly releasing their locks, allowing their jobs to be picked up by other workers.
    pub async fn force_unlock_workers(
        &self,
        worker_ids: &[&str],
    ) -> Result<(), GraphileWorkerError> {
        actions::force_unlock_workers(self, worker_ids).await
    }

    /// Runs database cleanup tasks to maintain performance.
    ///
    /// When `GcTaskIdentifiers` is included in the tasks and this `WorkerUtils` instance
    /// was created with task details (via `with_task_details`), the task identifiers
    /// known to this worker will be preserved and refreshed after cleanup.
    pub async fn cleanup(&self, tasks: &[CleanupTask]) -> Result<(), GraphileWorkerError> {
        maintenance::cleanup(self, tasks).await
    }

    /// Runs database migrations to ensure the schema is up to date.
    pub async fn migrate(&self) -> Result<(), MigrateError> {
        maintenance::migrate(self).await
    }
}
