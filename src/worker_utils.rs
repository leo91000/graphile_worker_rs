use std::sync::Arc;

use std::time::Duration;

use crate::recovery::{sweep_stale_workers, ActiveWorkerRow};
use crate::sql::add_job::{add_job, add_jobs, JobToAdd, RawJobSpec};
use crate::sql::task_identifiers::{get_tasks_details, SharedTaskDetails};
use crate::sql::worker_heartbeat::list_active_workers;
use crate::tracing::add_tracing_info;
use crate::{errors::GraphileWorkerError, DbJob, Job, JobSpec};
use graphile_worker_database::{Database, DbExecutor, DbExecutorArg, DbValue};
use graphile_worker_lifecycle_hooks::{BeforeJobScheduleContext, HookRegistry, JobScheduleResult};
use graphile_worker_migrations::{migrate, MigrateError};
use graphile_worker_task_handler::{BatchTaskHandler, TaskHandler};
use indoc::formatdoc;
use serde::Serialize;
use std::collections::HashSet;
use tracing::{debug, Span};

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
    DeletePermenantlyFailedJobs,
}

impl CleanupTask {
    pub(crate) async fn execute(
        &self,
        mut executor: impl DbExecutorArg,
        escaped_schema: &str,
        task_identifiers_to_keep: &[String],
    ) -> Result<(), GraphileWorkerError> {
        match self {
            CleanupTask::DeletePermenantlyFailedJobs => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_jobs jobs
                            where attempts = max_attempts
                            and locked_at is null;
                    "#
                );
                executor
                    .execute(&sql, graphile_worker_database::DbParams::new())
                    .await?;
            }
            CleanupTask::GcTaskIdentifiers => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_tasks tasks
                        where tasks.id not in (
                            select jobs.task_id from {escaped_schema}._private_jobs jobs
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
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_job_queues job_queues
                        where job_queues.id not in (
                            select jobs.job_queue_id from {escaped_schema}._private_jobs jobs
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

    async fn invoke_before_job_schedule(
        &self,
        identifier: &str,
        payload: serde_json::Value,
        spec: &JobSpec,
    ) -> Result<serde_json::Value, GraphileWorkerError> {
        let Some(hooks) = &self.hooks else {
            return Ok(payload);
        };

        let ctx = BeforeJobScheduleContext {
            identifier: identifier.to_string(),
            payload,
            spec: spec.clone(),
        };

        match hooks.intercept(ctx).await {
            JobScheduleResult::Continue(payload) => Ok(payload),
            JobScheduleResult::Skip => Err(GraphileWorkerError::JobScheduleSkipped),
            JobScheduleResult::Fail(msg) => Err(GraphileWorkerError::JobScheduleFailed(msg)),
        }
    }

    async fn prepare_batch_jobs<'a>(
        &self,
        jobs: Vec<(&'a str, serde_json::Value, &'a JobSpec)>,
    ) -> Result<(Vec<JobToAdd<'a>>, bool), GraphileWorkerError> {
        let mut jobs_to_add = Vec::with_capacity(jobs.len());
        let mut job_key_preserve_run_at = false;

        for (identifier, mut payload, spec) in jobs {
            add_tracing_info(&mut payload);

            let payload = self
                .invoke_before_job_schedule(identifier, payload, spec)
                .await?;

            job_key_preserve_run_at |= spec
                .job_key_mode()
                .as_ref()
                .is_some_and(|m| matches!(m, crate::JobKeyMode::PreserveRunAt));

            jobs_to_add.push(JobToAdd {
                identifier,
                payload,
                spec,
            });
        }

        Ok((jobs_to_add, job_key_preserve_run_at))
    }

    async fn analyze_jobs_after_large_batch(&self, job_count: usize) {
        if job_count < BULK_INSERT_ANALYZE_THRESHOLD {
            return;
        }

        let sql = formatdoc!(
            r#"
                analyze {escaped_schema}._private_jobs;
            "#,
            escaped_schema = self.escaped_schema
        );

        if let Err(error) = self
            .database
            .execute(&sql, graphile_worker_database::DbParams::new())
            .await
        {
            debug!(?error, "Failed to analyze jobs after large batch insert");
        }
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
        let identifier = T::IDENTIFIER;
        let mut payload = serde_json::to_value(payload)?;
        add_tracing_info(&mut payload);

        let span = Span::current();
        span.record("otel.name", identifier);
        span.record("messaging.destination.name", identifier);

        let payload = self
            .invoke_before_job_schedule(identifier, payload, &spec)
            .await?;
        add_job(
            &self.database,
            &self.escaped_schema,
            identifier,
            payload,
            spec,
            self.use_local_time,
        )
        .await
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
        let mut payload = serde_json::to_value(payload)?;
        add_tracing_info(&mut payload);

        let payload = self
            .invoke_before_job_schedule(identifier, payload, &spec)
            .await?;
        add_job(
            &self.database,
            &self.escaped_schema,
            identifier,
            payload,
            spec,
            self.use_local_time,
        )
        .await
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
        if jobs.is_empty() {
            return Ok(vec![]);
        }

        let identifier = T::IDENTIFIER;
        let mut job_inputs = Vec::with_capacity(jobs.len());
        for (payload, spec) in jobs {
            let payload_value = serde_json::to_value(payload)?;
            job_inputs.push((identifier, payload_value, *spec));
        }

        let (jobs_to_add, job_key_preserve_run_at) = self.prepare_batch_jobs(job_inputs).await?;

        let task_details = self.task_details.read().await;

        let added_jobs = add_jobs(
            &self.database,
            &self.escaped_schema,
            &jobs_to_add,
            &task_details,
            job_key_preserve_run_at,
            self.use_local_time,
        )
        .await?;
        drop(task_details);

        self.analyze_jobs_after_large_batch(jobs.len()).await;

        Ok(added_jobs)
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
        if jobs.is_empty() {
            return Ok(vec![]);
        }

        let job_inputs: Vec<_> = jobs
            .iter()
            .map(|job| (job.identifier.as_str(), job.payload.clone(), &job.spec))
            .collect();

        let (jobs_to_add, job_key_preserve_run_at) = self.prepare_batch_jobs(job_inputs).await?;

        let mut seen_identifiers = HashSet::with_capacity(jobs.len());
        let mut unique_identifiers = Vec::new();
        for job in jobs {
            if seen_identifiers.insert(job.identifier.as_str()) {
                unique_identifiers.push(job.identifier.clone());
            }
        }

        let task_details =
            get_tasks_details(&self.database, &self.escaped_schema, unique_identifiers).await?;

        let added_jobs = add_jobs(
            &self.database,
            &self.escaped_schema,
            &jobs_to_add,
            &task_details,
            job_key_preserve_run_at,
            self.use_local_time,
        )
        .await?;
        drop(task_details);

        self.analyze_jobs_after_large_batch(jobs.len()).await;

        Ok(added_jobs)
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
        let span = Span::current();
        span.record("messaging.batch.message_count", payloads.len());

        if payloads.is_empty() {
            return Err(GraphileWorkerError::JobScheduleFailed(
                "batch job payload must contain at least one item".to_string(),
            ));
        }

        let identifier = T::IDENTIFIER;
        span.record("otel.name", identifier);
        span.record("messaging.destination.name", identifier);

        let mut payload = serde_json::to_value(payloads)?;
        if let Some(items) = payload.as_array_mut() {
            for item in items {
                add_tracing_info(item);
            }
        }

        let payload = self
            .invoke_before_job_schedule(identifier, payload, &spec)
            .await?;

        let serde_json::Value::Array(payloads) = payload else {
            return Err(GraphileWorkerError::JobScheduleFailed(
                "before_job_schedule must return a JSON array for batch jobs".to_string(),
            ));
        };

        if payloads.is_empty() {
            return Err(GraphileWorkerError::JobScheduleFailed(
                "batch job payload must contain at least one item".to_string(),
            ));
        }

        add_job(
            &self.database,
            &self.escaped_schema,
            identifier,
            serde_json::Value::Array(payloads),
            spec,
            self.use_local_time,
        )
        .await
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
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.remove_job($1::text);
            "#,
            escaped_schema = self.escaped_schema
        );

        self.database
            .execute(&sql, vec![DbValue::Text(job_key.to_string())].into())
            .await?;

        Ok(())
    }

    /// Marks multiple jobs as completed.
    ///
    /// # Arguments
    /// * `ids` - Array of job IDs to mark as completed
    ///
    /// # Returns
    /// * `Result<Vec<DbJob>, GraphileWorkerError>` - The updated job records
    pub async fn complete_jobs(&self, ids: &[i64]) -> Result<Vec<DbJob>, GraphileWorkerError> {
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.complete_jobs($1::bigint[]);
            "#,
            escaped_schema = self.escaped_schema
        );

        let jobs = self
            .database
            .fetch_all(&sql, vec![DbValue::I64Array(ids.to_vec())].into())
            .await?
            .iter()
            .map(crate::sql::rows::db_job_from_row)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
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
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.permanently_fail_jobs($1::bigint[], $2::text);
            "#,
            escaped_schema = self.escaped_schema
        );

        let jobs = self
            .database
            .fetch_all(
                &sql,
                vec![
                    DbValue::I64Array(ids.to_vec()),
                    DbValue::Text(reason.to_string()),
                ]
                .into(),
            )
            .await?
            .iter()
            .map(crate::sql::rows::db_job_from_row)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
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
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.reschedule_jobs(
                    $1::bigint[],
                    run_at := $2::timestamptz,
                    priority := $3::int,
                    attempts := $4::int,
                    max_attempts := $5::int
                );
            "#,
            escaped_schema = self.escaped_schema
        );

        let jobs = self
            .database
            .fetch_all(
                &sql,
                vec![
                    DbValue::I64Array(ids.to_vec()),
                    DbValue::TimestampTzOpt(options.run_at),
                    DbValue::I32Opt(options.priority.map(i32::from)),
                    DbValue::I32Opt(options.attempts.map(i32::from)),
                    DbValue::I32Opt(options.max_attempts.map(i32::from)),
                ]
                .into(),
            )
            .await?
            .iter()
            .map(crate::sql::rows::db_job_from_row)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Lists workers registered in the heartbeat table.
    pub async fn list_active_workers(
        &self,
        sweep_threshold: Duration,
    ) -> Result<Vec<ActiveWorkerRow>, GraphileWorkerError> {
        list_active_workers(&self.database, &self.escaped_schema, sweep_threshold).await
    }

    /// Sweeps inactive workers and orphan locks, recovering their jobs.
    pub async fn sweep_stale_workers(
        &self,
        options: SweepStaleWorkersOptions,
    ) -> Result<SweepStaleWorkersResult, GraphileWorkerError> {
        sweep_stale_workers(
            &self.database,
            &self.escaped_schema,
            self.hooks.as_ref(),
            "worker_utils",
            options,
        )
        .await
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
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.force_unlock_workers($1::text[]);
            "#,
            escaped_schema = self.escaped_schema
        );

        self.database
            .execute(
                &sql,
                vec![DbValue::TextArray(
                    worker_ids
                        .iter()
                        .map(|worker_id| worker_id.to_string())
                        .collect(),
                )]
                .into(),
            )
            .await?;

        Ok(())
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
    ///     CleanupTask::DeletePermenantlyFailedJobs,
    ///     CleanupTask::GcTaskIdentifiers,
    ///     CleanupTask::GcJobQueues,
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cleanup(&self, tasks: &[CleanupTask]) -> Result<(), GraphileWorkerError> {
        let should_refresh_task_identifiers = tasks
            .iter()
            .any(|t| matches!(t, CleanupTask::GcTaskIdentifiers));

        if should_refresh_task_identifiers {
            let mut guard = self.task_details.write().await;
            let task_names = guard.task_names();

            for task in tasks {
                task.execute(&self.database, &self.escaped_schema, &task_names)
                    .await?;
            }

            let refreshed =
                get_tasks_details(&self.database, &self.escaped_schema, task_names).await?;
            *guard = refreshed;
            return Ok(());
        }

        for task in tasks {
            task.execute(&self.database, &self.escaped_schema, &[])
                .await?;
        }

        Ok(())
    }

    /// Runs database migrations to ensure the schema is up to date.
    ///
    /// Automatically called when initializing a worker, but can
    /// also be called manually if needed.
    ///
    /// # Returns
    /// * `Result<(), MigrateError>` - Ok if migrations completed successfully
    pub async fn migrate(&self) -> Result<(), MigrateError> {
        migrate(&self.database, &self.escaped_schema).await?;
        Ok(())
    }
}
