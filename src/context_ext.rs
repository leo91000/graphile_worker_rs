use crate::errors::GraphileWorkerError;
use crate::sql::add_job::types::RawJobSpec;
use crate::worker_utils::client::WorkerUtils;
use crate::{Job, JobSpec, TaskHandler};
use graphile_worker_ctx::WorkerContext;
use graphile_worker_task_handler::BatchTaskHandler;
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

    /// Add multiple typed jobs in a single batch operation.
    fn add_jobs<T: TaskHandler + Clone + 'static>(
        &self,
        jobs: &[(T, &JobSpec)],
    ) -> impl core::future::Future<Output = Result<Vec<Job>, GraphileWorkerError>> + Send;

    /// Add multiple raw jobs in a single batch operation.
    fn add_raw_jobs(
        &self,
        jobs: &[RawJobSpec],
    ) -> impl core::future::Future<Output = Result<Vec<Job>, GraphileWorkerError>> + Send;

    /// Add a batch job from within a task handler.
    fn add_batch_job<T: BatchTaskHandler + 'static>(
        &self,
        payloads: Vec<T>,
        spec: JobSpec,
    ) -> impl core::future::Future<Output = Result<Job, GraphileWorkerError>> + Send;
}

impl WorkerContextExt for WorkerContext {
    fn utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.database().clone(), self.schema().clone())
            .with_use_local_time(self.use_local_time())
            .with_task_details(self.task_details().clone())
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

    async fn add_jobs<T: TaskHandler + Clone + 'static>(
        &self,
        jobs: &[(T, &JobSpec)],
    ) -> Result<Vec<Job>, GraphileWorkerError> {
        self.utils().add_jobs(jobs).await
    }

    async fn add_raw_jobs(&self, jobs: &[RawJobSpec]) -> Result<Vec<Job>, GraphileWorkerError> {
        self.utils().add_raw_jobs(jobs).await
    }

    async fn add_batch_job<T: BatchTaskHandler + 'static>(
        &self,
        payloads: Vec<T>,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        self.utils().add_batch_job(payloads, spec).await
    }
}
