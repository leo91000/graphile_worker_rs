use graphile_worker_task_handler::{BatchTaskHandler, TaskHandler};
use serde::Serialize;

use super::add;
use super::client::WorkerUtils;
use crate::{errors::GraphileWorkerError, Job, JobSpec};
use graphile_worker_queries::add_job::types::RawJobSpec;

impl WorkerUtils {
    /// Adds a job to the queue with type safety.
    ///
    /// Uses the TaskHandler trait to ensure that the job identifier
    /// and payload type match, providing compile-time type safety.
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerUtils, JobSpec, TaskHandler, WorkerContext, IntoTaskHandlerResult};
    /// # use graphile_worker::errors::GraphileWorkerError;
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Deserialize, Serialize)]
    /// # struct MyTask { data: String }
    /// # impl TaskHandler for MyTask {
    /// #     const IDENTIFIER: &'static str = "my_task";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult { Ok::<(), String>(()) }
    /// # }
    /// # async fn example(utils: &WorkerUtils) -> Result<(), GraphileWorkerError> {
    /// let job = utils.add_job(
    ///     MyTask { data: "hello".to_string() },
    ///     JobSpec::default()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        "add_job",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_job",
            messaging.destination.name = tracing::field::Empty,
            otel.name = tracing::field::Empty
        )
    )]
    pub async fn add_job<T: TaskHandler>(
        &self,
        payload: T,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        add::add_job(self, payload, spec).await
    }

    /// Adds a job to the queue with a raw identifier and payload.
    ///
    /// Doesn't require the task handler to be defined at compile time,
    /// allowing for dynamic job creation. However, lacks the compile-time type safety
    /// of `add_job`.
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerUtils, JobSpec};
    /// # use graphile_worker::errors::GraphileWorkerError;
    /// # use serde_json::json;
    /// # async fn example(utils: &WorkerUtils) -> Result<(), GraphileWorkerError> {
    /// let job = utils.add_raw_job(
    ///     "send_email",
    ///     json!({ "to": "user@example.com", "subject": "Hello" }),
    ///     JobSpec::default()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        "add_raw_job",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_job",
            messaging.destination.name = identifier,
            otel.name = identifier
        )
    )]
    pub async fn add_raw_job<P>(
        &self,
        identifier: &str,
        payload: P,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError>
    where
        P: Serialize,
    {
        add::add_raw_job(self, identifier, payload, spec).await
    }

    /// Adds multiple jobs of the same type to the queue in a single batch operation.
    ///
    /// This is more efficient than calling `add_job` multiple times when you need to
    /// add many jobs of the same type, as it uses a single database round trip.
    ///
    /// # Limitations
    /// * `job_key_mode: UnsafeDedupe` is not supported in batch operations
    /// * `job_key_mode` is applied uniformly: if any job uses `PreserveRunAt`, it applies
    ///   to all jobs in the batch. For per-job `job_key_mode` control, use `add_job` individually.
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
        &self,
        jobs: &[(T, &JobSpec)],
    ) -> Result<Vec<Job>, GraphileWorkerError> {
        add::add_jobs(self, jobs).await
    }

    /// Adds multiple jobs with raw identifiers and payloads in a single batch operation.
    ///
    /// This allows adding jobs of different types in a single batch, but without
    /// compile-time type safety. More efficient than multiple `add_raw_job` calls.
    ///
    /// # Limitations
    /// * `job_key_mode: UnsafeDedupe` is not supported in batch operations
    /// * `job_key_mode` is applied uniformly: if any job uses `PreserveRunAt`, it applies
    ///   to all jobs in the batch. For per-job `job_key_mode` control, use `add_raw_job` individually.
    #[tracing::instrument(
        "add_raw_jobs",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_jobs",
            messaging.batch.message_count = jobs.len()
        )
    )]
    pub async fn add_raw_jobs(&self, jobs: &[RawJobSpec]) -> Result<Vec<Job>, GraphileWorkerError> {
        add::add_raw_jobs(self, jobs).await
    }

    /// Adds a batch job to the queue with type safety.
    ///
    /// The database payload is a JSON array of `T` items. Register the same
    /// identifier with [`WorkerOptions::define_batch_job`](crate::WorkerOptions::define_batch_job)
    /// so the worker can run the batch and retry only failed items after partial
    /// success.
    #[tracing::instrument(
        "add_batch_job",
        skip_all,
        fields(
            messaging.system = "graphile-worker",
            messaging.operation.name = "add_batch_job",
            messaging.batch.message_count = tracing::field::Empty,
            messaging.destination.name = tracing::field::Empty,
            otel.name = tracing::field::Empty
        )
    )]
    pub async fn add_batch_job<T: BatchTaskHandler>(
        &self,
        payloads: Vec<T>,
        spec: JobSpec,
    ) -> Result<Job, GraphileWorkerError> {
        add::add_batch_job(self, payloads, spec).await
    }
}
