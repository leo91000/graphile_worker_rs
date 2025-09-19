use crate::errors::GraphileWorkerError;
use crate::worker_utils::{CleanupTask, RescheduleJobOptions, WorkerUtils};
use crate::{DbJob, Job, JobSpec, TaskHandler};
use graphile_worker_migrations::MigrateError;
use graphile_worker_ctx::WorkerContext;
use serde::Serialize;

/// Convenience helpers available from task handlers via `WorkerContext`.
///
/// This trait is implemented for `WorkerContext` in this crate, so user code
/// can call `ctx.utils()` or `ctx.add_job(...)` from inside task handlers
/// without manually wiring the schema.
pub trait WorkerContextExt {
    /// Create a `WorkerUtils` bound to the same pool and schema as the running worker.
    fn utils(&self) -> WorkerUtils;

    /// Add a typed job (using the TaskHandler identifier) from within a task handler.
    fn add_job<T: TaskHandler + 'static>(
        &self,
        payload: T,
        spec: JobSpec,
    ) -> impl core::future::Future<Output = Result<Job, GraphileWorkerError>> + Send;

    /// Add a raw job by identifier from within a task handler.
    fn add_raw_job<P: Serialize + Send + 'static>(
        &self,
        identifier: &str,
        payload: P,
        spec: JobSpec,
    ) -> impl core::future::Future<Output = Result<Job, GraphileWorkerError>> + Send;

    /// Remove a job by job key.
    fn remove_job(&self, job_key: &str)
        -> impl core::future::Future<Output = Result<(), GraphileWorkerError>> + Send;

    /// Mark jobs as completed.
    fn complete_jobs(&self, ids: &[i64])
        -> impl core::future::Future<Output = Result<Vec<DbJob>, GraphileWorkerError>> + Send;

    /// Permanently fail jobs with a reason.
    fn permanently_fail_jobs(
        &self,
        ids: &[i64],
        reason: &str,
    ) -> impl core::future::Future<Output = Result<Vec<DbJob>, GraphileWorkerError>> + Send;

    /// Reschedule jobs with new options.
    fn reschedule_jobs(
        &self,
        ids: &[i64],
        options: RescheduleJobOptions,
    ) -> impl core::future::Future<Output = Result<Vec<DbJob>, GraphileWorkerError>> + Send;

    /// Force unlock workers in DB.
    fn force_unlock_workers(
        &self,
        worker_ids: &[&str],
    ) -> impl core::future::Future<Output = Result<(), GraphileWorkerError>> + Send;

    /// Run cleanup tasks.
    fn cleanup(&self, tasks: &[CleanupTask])
        -> impl core::future::Future<Output = Result<(), GraphileWorkerError>> + Send;

    /// Run migrations for the schema.
    fn migrate(&self) -> impl core::future::Future<Output = Result<(), MigrateError>> + Send;
}

impl WorkerContextExt for WorkerContext {
    fn utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.pg_pool().clone(), self.escaped_schema().to_string())
    }

    async fn add_job<T: TaskHandler + 'static>(
        &self,
        payload: T,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        self.utils().add_job(payload, spec).await
    }

    async fn add_raw_job<P: Serialize + Send + 'static>(
        &self,
        identifier: &str,
        payload: P,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        self.utils().add_raw_job(identifier, payload, spec).await
    }

    async fn remove_job(&self, job_key: &str) -> Result<(), GraphileWorkerError> {
        self.utils().remove_job(job_key).await
    }

    async fn complete_jobs(&self, ids: &[i64])
        -> Result<Vec<DbJob>, GraphileWorkerError>
    {
        self.utils().complete_jobs(ids).await
    }

    async fn permanently_fail_jobs(
        &self,
        ids: &[i64],
        reason: &str,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        self.utils().permanently_fail_jobs(ids, reason).await
    }

    async fn reschedule_jobs(
        &self,
        ids: &[i64],
        options: RescheduleJobOptions,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        self.utils().reschedule_jobs(ids, options).await
    }

    async fn force_unlock_workers(
        &self,
        worker_ids: &[&str],
    ) -> Result<(), GraphileWorkerError> {
        self.utils().force_unlock_workers(worker_ids).await
    }

    async fn cleanup(&self, tasks: &[CleanupTask])
        -> Result<(), GraphileWorkerError>
    {
        self.utils().cleanup(tasks).await
    }

    async fn migrate(&self) -> Result<(), MigrateError> {
        self.utils().migrate().await
    }
}
