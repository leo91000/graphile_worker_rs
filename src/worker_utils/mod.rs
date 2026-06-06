use std::sync::Arc;

use std::time::Duration;

use crate::recovery::{ActiveWorkerRow, WorkerRecoveryConfig};
use crate::sql::add_job::RawJobSpec;
use crate::sql::dynamic::{DynamicSchema, PrivateTable};
use crate::sql::task_identifiers::SharedTaskDetails;
use crate::{errors::GraphileWorkerError, DbJob, Job, JobSpec};
use graphile_worker_database::{Database, DbExecutorArg, DbValue};
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_migrations::MigrateError;
use graphile_worker_task_handler::{BatchTaskHandler, TaskHandler};
use indoc::formatdoc;
use serde::Serialize;

mod actions;
mod add;
mod maintenance;

const BULK_INSERT_ANALYZE_THRESHOLD: usize = 10_000;

pub use crate::recovery::{SweepStaleWorkersOptions, SweepStaleWorkersResult};

/// Types of database cleanup tasks that can be performed on the Graphile Worker schema.
///
/// These tasks help maintain database performance by removing unused records and
/// reducing database size over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupTask {
    /// Removes task identifier records that are no longer referenced by any jobs.
    /// This helps keep the `_private_tasks` table clean and smaller.
    ///
    /// **Note**: When using `WorkerUtils::cleanup()` from a worker, task identifiers
    /// that the worker knows about will be preserved to support horizontal scaling.
    GcTaskIdentifiers,

    /// Removes job queue records that are no longer referenced by any jobs.
    /// This helps keep the `_private_job_queues` table clean and smaller.
    GcJobQueues,

    /// Removes jobs that have reached their maximum retry attempts and are no longer locked.
    /// This helps clean up permanently failed jobs that will never be processed again.
    DeletePermanentlyFailedJobs,

    /// Deprecated misspelling retained for source compatibility.
    #[deprecated(
        since = "0.13.2",
        note = "use CleanupTask::DeletePermanentlyFailedJobs instead"
    )]
    DeletePermenantlyFailedJobs,
}

impl CleanupTask {
    #[allow(deprecated)]
    pub(crate) async fn execute(
        &self,
        mut executor: impl DbExecutorArg,
        escaped_schema: &str,
        task_identifiers_to_keep: &[String],
    ) -> Result<(), GraphileWorkerError> {
        match self {
            CleanupTask::DeletePermanentlyFailedJobs | CleanupTask::DeletePermenantlyFailedJobs => {
                let jobs = DynamicSchema::new(escaped_schema).private_table(PrivateTable::Jobs);
                let sql = formatdoc!(
                    r#"
                        delete from {jobs} jobs
                            where attempts = max_attempts
                            and locked_at is null;
                    "#
                );
                executor
                    .execute(&sql, graphile_worker_database::DbParams::new())
                    .await?;
            }
            CleanupTask::GcTaskIdentifiers => {
                let jobs = DynamicSchema::new(escaped_schema).private_table(PrivateTable::Jobs);
                let tasks = DynamicSchema::new(escaped_schema).private_table(PrivateTable::Tasks);
                let sql = formatdoc!(
                    r#"
                        delete from {tasks} tasks
                        where tasks.id not in (
                            select jobs.task_id from {jobs} jobs
                        )
                        and tasks.identifier <> all ($1::text[]);
                    "#
                );
                executor
                    .execute(
                        &sql,
                        vec![DbValue::TextArray(task_identifiers_to_keep.to_vec())].into(),
                    )
                    .await?;
            }
            CleanupTask::GcJobQueues => {
                let jobs = DynamicSchema::new(escaped_schema).private_table(PrivateTable::Jobs);
                let job_queues =
                    DynamicSchema::new(escaped_schema).private_table(PrivateTable::JobQueues);
                let sql = formatdoc!(
                    r#"
                        delete from {job_queues} job_queues
                        where job_queues.id not in (
                            select jobs.job_queue_id from {jobs} jobs
                        );
                    "#
                );
                executor
                    .execute(&sql, graphile_worker_database::DbParams::new())
                    .await?;
            }
        }

        Ok(())
    }
}

/// Options for rescheduling jobs.
///
/// This struct allows you to specify various parameters when rescheduling jobs,
/// such as when the job should run, its priority, and how many retry attempts it should have.
/// All fields are optional, and only specified fields will be updated.
#[derive(Default, Debug)]
pub struct RescheduleJobOptions {
    /// When the job should be executed. If not specified, jobs will be scheduled to run immediately.
    pub run_at: Option<chrono::DateTime<chrono::Utc>>,

    /// The job's priority. Higher numbers indicate higher priority (runs sooner).
    /// Default priority is 0.
    pub priority: Option<i16>,

    /// How many times the job has been attempted.
    /// Normally this should not be manually set.
    pub attempts: Option<i16>,

    /// Maximum number of retry attempts before the job is considered permanently failed.
    /// Default is 25 attempts.
    pub max_attempts: Option<i16>,
}

/// The WorkerUtils struct provides a set of utility methods for managing jobs.
///
/// This is the primary interface for adding jobs to the queue, managing existing jobs,
/// performing maintenance tasks, and migrating the database schema.
#[derive(Clone)]
pub struct WorkerUtils {
    /// Database connection pool
    database: Database,

    /// SQL-escaped schema name where Graphile Worker tables are located
    escaped_schema: String,

    /// Optional lifecycle hooks for intercepting job scheduling
    hooks: Option<Arc<HookRegistry>>,

    /// Shared task details for refreshing after GcTaskIdentifiers cleanup
    task_details: SharedTaskDetails,

    /// Whether to use local application time (true) or database time (false) for timestamps
    use_local_time: bool,
}

impl WorkerUtils {
    /// Creates a new instance of WorkerUtils.
    ///
    /// # Arguments
    /// * `database` - Database connection handle
    /// * `escaped_schema` - The escaped name of the schema where Graphile Worker tables are stored
    ///
    /// # Returns
    /// A new WorkerUtils instance
    pub fn new(database: impl Into<Database>, escaped_schema: String) -> Self {
        Self {
            database: database.into(),
            escaped_schema,
            hooks: None,
            task_details: SharedTaskDetails::default(),
            use_local_time: false,
        }
    }

    /// Adds lifecycle hooks to this WorkerUtils instance.
    pub fn with_hooks(mut self, hooks: Arc<HookRegistry>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Adds task details to this WorkerUtils instance.
    ///
    /// When task_details is provided, cleanup operations that include `GcTaskIdentifiers`
    /// will automatically refresh the task details to ensure the worker can still pick
    /// up jobs after task identifiers are garbage collected.
    pub fn with_task_details(mut self, task_details: SharedTaskDetails) -> Self {
        self.task_details = task_details;
        self
    }

    /// Sets whether to use local application time or database time for timestamps.
    ///
    /// When `use_local_time` is true, the application's `Utc::now()` is used for timestamps,
    /// which can help handle clock drift between the application server and database server.
    /// When false (default), PostgreSQL's `now()` is used instead.
    pub fn with_use_local_time(mut self, use_local_time: bool) -> Self {
        self.use_local_time = use_local_time;
        self
    }

    /// Adds a job to the queue with type safety.
    ///
    /// Uses the TaskHandler trait to ensure that the job identifier
    /// and payload type match, providing compile-time type safety.
    ///
    /// # Arguments
    /// * `payload` - The job payload, which must match the type specified in the task handler
    /// * `spec` - Job specification (priority, queue, etc.)
    ///
    /// # Returns
    /// * `Result<Job, GraphileWorkerError>` - The created job or an error
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
    /// # Arguments
    /// * `identifier` - The task identifier string
    /// * `payload` - The job payload (any serializable type)
    /// * `spec` - Job specification (priority, queue, etc.)
    ///
    /// # Returns
    /// * `Result<Job, GraphileWorkerError>` - The created job or an error
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
    /// # Arguments
    /// * `jobs` - Slice of tuples containing payloads and job specifications
    ///
    /// # Returns
    /// * `Result<Vec<Job>, GraphileWorkerError>` - The created jobs or an error
    ///
    /// # Limitations
    /// * `job_key_mode: UnsafeDedupe` is not supported in batch operations
    /// * `job_key_mode` is applied uniformly: if any job uses `PreserveRunAt`, it applies
    ///   to all jobs in the batch. For per-job `job_key_mode` control, use `add_job` individually.
    ///
    /// # Hook Failure Behavior
    /// If the `before_job_schedule` hook fails for any job in the batch, the entire
    /// batch operation fails and no jobs are added. For partial success semantics,
    /// use `add_job` individually in a loop with your own error handling.
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerUtils, JobSpec, TaskHandler, WorkerContext, IntoTaskHandlerResult};
    /// # use graphile_worker::errors::GraphileWorkerError;
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Clone, Deserialize, Serialize)]
    /// # struct SendEmail { to: String }
    /// # impl TaskHandler for SendEmail {
    /// #     const IDENTIFIER: &'static str = "send_email";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult { Ok::<(), String>(()) }
    /// # }
    /// # async fn example(utils: &WorkerUtils) -> Result<(), GraphileWorkerError> {
    /// let spec = JobSpec::default();
    /// let jobs = utils.add_jobs::<SendEmail>(&[
    ///     (SendEmail { to: "user1@example.com".into() }, &spec),
    ///     (SendEmail { to: "user2@example.com".into() }, &spec),
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
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
    /// # Arguments
    /// * `jobs` - Slice of `RawJobSpec` containing identifiers, payloads, and specifications
    ///
    /// # Returns
    /// * `Result<Vec<Job>, GraphileWorkerError>` - The created jobs or an error
    ///
    /// # Limitations
    /// * `job_key_mode: UnsafeDedupe` is not supported in batch operations
    /// * `job_key_mode` is applied uniformly: if any job uses `PreserveRunAt`, it applies
    ///   to all jobs in the batch. For per-job `job_key_mode` control, use `add_raw_job` individually.
    ///
    /// # Hook Failure Behavior
    /// If the `before_job_schedule` hook fails for any job in the batch, the entire
    /// batch operation fails and no jobs are added. For partial success semantics,
    /// use `add_raw_job` individually in a loop with your own error handling.
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerUtils, JobSpec, RawJobSpec};
    /// # use graphile_worker::errors::GraphileWorkerError;
    /// # use serde_json::json;
    /// # async fn example(utils: &WorkerUtils) -> Result<(), GraphileWorkerError> {
    /// let jobs = utils.add_raw_jobs(&[
    ///     RawJobSpec {
    ///         identifier: "send_email".into(),
    ///         payload: json!({ "to": "user@example.com" }),
    ///         spec: JobSpec::default(),
    ///     },
    ///     RawJobSpec {
    ///         identifier: "process_payment".into(),
    ///         payload: json!({ "amount": 100 }),
    ///         spec: JobSpec::default(),
    ///     },
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Removes a job from the queue by its job key.
    ///
    /// Useful for cancelling scheduled jobs that haven't run yet.
    ///
    /// # Arguments
    /// * `job_key` - The unique job key that was assigned when the job was created
    ///
    /// # Returns
    /// * `Result<(), GraphileWorkerError>` - Ok if the job was removed successfully
    pub async fn remove_job(&self, job_key: &str) -> Result<(), GraphileWorkerError> {
        actions::remove_job(self, job_key).await
    }

    /// Marks multiple jobs as completed.
    ///
    /// # Arguments
    /// * `ids` - Array of job IDs to mark as completed
    ///
    /// # Returns
    /// * `Result<Vec<DbJob>, GraphileWorkerError>` - The updated job records
    pub async fn complete_jobs(&self, ids: &[i64]) -> Result<Vec<DbJob>, GraphileWorkerError> {
        actions::complete_jobs(self, ids).await
    }

    /// Marks multiple jobs as permanently failed with a reason.
    ///
    /// # Arguments
    /// * `ids` - Array of job IDs to mark as permanently failed
    /// * `reason` - The reason for the failure, which will be recorded in the error field
    ///
    /// # Returns
    /// * `Result<Vec<DbJob>, GraphileWorkerError>` - The updated job records
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
    ///
    /// # Arguments
    /// * `ids` - Array of job IDs to reschedule
    /// * `options` - Options for rescheduling (run_at, priority, attempts, max_attempts)
    ///
    /// # Returns
    /// * `Result<Vec<DbJob>, GraphileWorkerError>` - The updated job records
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
    ///
    /// # Arguments
    /// * `worker_ids` - Array of worker IDs to force unlock
    ///
    /// # Returns
    /// * `Result<(), GraphileWorkerError>` - Ok if workers were successfully unlocked
    pub async fn force_unlock_workers(
        &self,
        worker_ids: &[&str],
    ) -> Result<(), GraphileWorkerError> {
        actions::force_unlock_workers(self, worker_ids).await
    }

    /// Runs database cleanup tasks to maintain performance.
    ///
    /// Executes the specified cleanup tasks, which helps keep
    /// the database size manageable and improves query performance.
    ///
    /// When `GcTaskIdentifiers` is included in the tasks and this `WorkerUtils` instance
    /// was created with task details (via `with_task_details`), the task identifiers
    /// known to this worker will be preserved (not deleted), supporting horizontal scaling.
    /// Additionally, task details will be refreshed after cleanup.
    ///
    /// # Arguments
    /// * `tasks` - Array of cleanup tasks to perform
    ///
    /// # Returns
    /// * `Result<(), GraphileWorkerError>` - Ok if all cleanup tasks completed successfully
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerUtils;
    /// # use graphile_worker::worker_utils::CleanupTask;
    /// # use graphile_worker::errors::GraphileWorkerError;
    /// # async fn example(utils: &WorkerUtils) -> Result<(), GraphileWorkerError> {
    /// utils.cleanup(&[
    ///     CleanupTask::DeletePermanentlyFailedJobs,
    ///     CleanupTask::GcTaskIdentifiers,
    ///     CleanupTask::GcJobQueues,
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cleanup(&self, tasks: &[CleanupTask]) -> Result<(), GraphileWorkerError> {
        maintenance::cleanup(self, tasks).await
    }

    /// Runs database migrations to ensure the schema is up to date.
    ///
    /// Automatically called when initializing a worker, but can
    /// also be called manually if needed.
    ///
    /// # Returns
    /// * `Result<(), MigrateError>` - Ok if migrations completed successfully
    pub async fn migrate(&self) -> Result<(), MigrateError> {
        maintenance::migrate(self).await
    }
}
