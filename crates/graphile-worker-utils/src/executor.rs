use std::time::Duration;

use graphile_worker_database::DbExecutorArg;
use graphile_worker_job::{DbJob, Job};
use graphile_worker_job_spec::JobSpec;
use graphile_worker_queries::add_job::types::RawJobSpec;
use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_recovery::ActiveWorkerRow;
use graphile_worker_task_handler::{BatchTaskHandler, TaskHandler};
use serde::Serialize;

use super::client::WorkerUtils;
use super::types::RescheduleJobOptions;
use super::{actions, add, maintenance};

/// A scoped `WorkerUtils` facade that routes operations through a caller-provided executor.
///
/// Use [`WorkerUtils::with_executor`] when job operations must participate in an existing
/// transaction. The caller owns the executor lifecycle and remains responsible for committing or
/// rolling back transactions.
///
/// Maintenance operations that manage their own transaction or shared state (`cleanup`,
/// `migrate`, and stale-worker sweeps) remain available only on [`WorkerUtils`].
pub struct WorkerUtilsWithExecutor<E> {
    utils: WorkerUtils,
    executor: E,
}

impl<E> WorkerUtilsWithExecutor<E>
where
    E: DbExecutorArg,
{
    pub(super) fn new(utils: WorkerUtils, executor: E) -> Self {
        Self { utils, executor }
    }

    /// Returns the configured `WorkerUtils`, releasing the injected executor.
    pub fn into_inner(self) -> WorkerUtils {
        self.utils
    }

    /// Adds a job to the queue with type safety.
    #[tracing::instrument(
        "add_job",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_job"
        )
    )]
    pub async fn add_job<T: TaskHandler>(
        &mut self,
        payload: T,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        add::add_job(&self.utils, &mut self.executor, payload, spec).await
    }

    /// Adds a job to the queue with a raw identifier and payload.
    #[tracing::instrument(
        "add_raw_job",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_job"
        )
    )]
    pub async fn add_raw_job<P>(
        &mut self,
        identifier: &str,
        payload: P,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError>
    where
        P: Serialize,
    {
        add::add_raw_job(&self.utils, &mut self.executor, identifier, payload, spec).await
    }

    /// Adds multiple jobs of the same type in one database operation.
    #[tracing::instrument(
        "add_jobs",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_jobs",
            messaging.batch.message_count = jobs.len()
        )
    )]
    pub async fn add_jobs<T: TaskHandler + Clone>(
        &mut self,
        jobs: &[(T, &JobSpec)],
    ) -> Result<Vec<Job>, GraphileWorkerError> {
        add::add_jobs(&self.utils, &mut self.executor, jobs).await
    }

    /// Adds multiple jobs with raw identifiers and payloads in one database operation.
    #[tracing::instrument(
        "add_raw_jobs",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_jobs",
            messaging.batch.message_count = jobs.len()
        )
    )]
    pub async fn add_raw_jobs(
        &mut self,
        jobs: &[RawJobSpec],
    ) -> Result<Vec<Job>, GraphileWorkerError> {
        add::add_raw_jobs(&self.utils, &mut self.executor, jobs).await
    }

    /// Adds a batch job to the queue with type safety.
    #[tracing::instrument(
        "add_batch_job",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_batch_job",
            messaging.batch.message_count = payloads.len()
        )
    )]
    pub async fn add_batch_job<T: BatchTaskHandler>(
        &mut self,
        payloads: Vec<T>,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        add::add_batch_job(&self.utils, &mut self.executor, payloads, spec).await
    }

    /// Removes a job from the queue by its job key.
    pub async fn remove_job(&mut self, job_key: &str) -> Result<(), GraphileWorkerError> {
        actions::remove_job(&self.utils, &mut self.executor, job_key).await
    }

    /// Marks multiple jobs as completed.
    pub async fn complete_jobs(&mut self, ids: &[i64]) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::complete_jobs(&self.utils, &mut self.executor, ids).await
    }

    /// Marks multiple jobs as permanently failed with a reason.
    pub async fn permanently_fail_jobs(
        &mut self,
        ids: &[i64],
        reason: &str,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::permanently_fail_jobs(&self.utils, &mut self.executor, ids, reason).await
    }

    /// Reschedules multiple jobs with new parameters.
    pub async fn reschedule_jobs(
        &mut self,
        ids: &[i64],
        options: RescheduleJobOptions,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::reschedule_jobs(&self.utils, &mut self.executor, ids, options).await
    }

    /// Lists workers registered in the heartbeat table.
    pub async fn list_active_workers(
        &mut self,
        sweep_threshold: Duration,
    ) -> Result<Vec<ActiveWorkerRow>, GraphileWorkerError> {
        maintenance::list_active_workers(&self.utils, &mut self.executor, sweep_threshold).await
    }

    /// Force unlocks worker records in the database.
    pub async fn force_unlock_workers(
        &mut self,
        worker_ids: &[&str],
    ) -> Result<(), GraphileWorkerError> {
        actions::force_unlock_workers(&self.utils, &mut self.executor, worker_ids).await
    }
}
