use std::sync::Arc;

use crate::sql::task_identifiers::{get_tasks_details, TaskDetails};
use crate::tracing::add_tracing_info;
use crate::{errors::GraphileWorkerError, sql::add_job::add_job, DbJob, Job, JobSpec};
use graphile_worker_lifecycle_hooks::{
    BeforeJobScheduleContext, JobScheduleResult, TypeErasedHooks,
};
use graphile_worker_migrations::{migrate, MigrateError};
use graphile_worker_task_handler::TaskHandler;
use indoc::formatdoc;
use serde::Serialize;
use sqlx::{PgExecutor, PgPool};
use tokio::sync::RwLock;
use tracing::Span;

/// Types of database cleanup tasks that can be performed on the Graphile Worker schema.
///
/// These tasks help maintain database performance by removing unused records and
/// reducing database size over time.
#[derive(Debug, Clone, Copy)]
pub enum CleanupTask {
    /// Removes task identifier records that are no longer referenced by any jobs.
    /// This helps keep the `_private_tasks` table clean and smaller.
    GcTaskIdentifiers,

    /// Removes job queue records that are no longer referenced by any jobs.
    /// This helps keep the `_private_job_queues` table clean and smaller.
    GcJobQueues,

    /// Removes jobs that have reached their maximum retry attempts and are no longer locked.
    /// This helps clean up permanently failed jobs that will never be processed again.
    DeletePermenantlyFailedJobs,
}

impl CleanupTask {
    /// Executes the cleanup task on the database.
    ///
    /// # Arguments
    /// * `executor` - A PostgreSQL executor to run the query on
    /// * `escaped_schema` - The escaped name of the schema where Graphile Worker tables are stored
    ///
    /// # Returns
    /// * `Result<(), GraphileWorkerError>` - Ok if the task completed successfully
    pub async fn execute<'e>(
        &self,
        executor: impl PgExecutor<'e>,
        escaped_schema: &str,
    ) -> Result<(), GraphileWorkerError> {
        match self {
            CleanupTask::DeletePermenantlyFailedJobs => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_jobs jobs
                            where attempts = max_attempts
                            and locked_at is null;
                    "#,
                    escaped_schema = escaped_schema
                );

                sqlx::query(&sql).execute(executor).await?;
            }
            CleanupTask::GcTaskIdentifiers => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_tasks tasks
                        where tasks.id not in (
                            select jobs.task_id from {escaped_schema}._private_jobs jobs
                        );
                    "#,
                    escaped_schema = escaped_schema
                );

                sqlx::query(&sql).execute(executor).await?;
            }
            CleanupTask::GcJobQueues => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_job_queues job_queues
                        where job_queues.id not in (
                            select jobs.job_queue_id from {escaped_schema}._private_jobs jobs
                        );
                    "#,
                    escaped_schema = escaped_schema
                );

                sqlx::query(&sql)
                    .execute(executor)
                    .await
                    .expect("Failed to run gc_job_queues");
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
    pg_pool: PgPool,

    /// SQL-escaped schema name where Graphile Worker tables are located
    escaped_schema: String,

    /// Optional lifecycle hooks for intercepting job scheduling
    hooks: Option<Arc<TypeErasedHooks>>,

    /// Shared task details for refreshing after GcTaskIdentifiers cleanup
    task_details: Option<Arc<RwLock<TaskDetails>>>,
}

impl WorkerUtils {
    /// Creates a new instance of WorkerUtils.
    ///
    /// # Arguments
    /// * `pg_pool` - PostgreSQL connection pool
    /// * `escaped_schema` - The escaped name of the schema where Graphile Worker tables are stored
    ///
    /// # Returns
    /// A new WorkerUtils instance
    pub fn new(pg_pool: PgPool, escaped_schema: String) -> Self {
        Self {
            pg_pool,
            escaped_schema,
            hooks: None,
            task_details: None,
        }
    }

    /// Adds lifecycle hooks to this WorkerUtils instance.
    pub fn with_hooks(mut self, hooks: Arc<TypeErasedHooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Adds task details to this WorkerUtils instance.
    ///
    /// When task_details is provided, cleanup operations that include `GcTaskIdentifiers`
    /// will automatically refresh the task details to ensure the worker can still pick
    /// up jobs after task identifiers are garbage collected.
    pub fn with_task_details(mut self, task_details: Arc<RwLock<TaskDetails>>) -> Self {
        self.task_details = Some(task_details);
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

        let mut current_payload = payload;
        for hook in &hooks.before_job_schedule {
            let ctx = BeforeJobScheduleContext {
                identifier: identifier.to_string(),
                payload: current_payload,
                spec: spec.clone(),
            };
            match hook(ctx).await {
                JobScheduleResult::Continue(new_payload) => {
                    current_payload = new_payload;
                }
                JobScheduleResult::Skip => {
                    return Err(GraphileWorkerError::JobScheduleSkipped);
                }
                JobScheduleResult::Fail(msg) => {
                    return Err(GraphileWorkerError::JobScheduleFailed(msg));
                }
            }
        }
        Ok(current_payload)
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
            &self.pg_pool,
            &self.escaped_schema,
            identifier,
            payload,
            spec,
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
            &self.pg_pool,
            &self.escaped_schema,
            identifier,
            payload,
            spec,
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

        sqlx::query(&sql)
            .bind(job_key)
            .execute(&self.pg_pool)
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

        let jobs = sqlx::query_as(&sql)
            .bind(ids)
            .fetch_all(&self.pg_pool)
            .await?;

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

        let jobs = sqlx::query_as(&sql)
            .bind(ids)
            .bind(reason)
            .fetch_all(&self.pg_pool)
            .await?;

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

        let jobs = sqlx::query_as(&sql)
            .bind(ids)
            .bind(options.run_at)
            .bind(options.priority)
            .bind(options.attempts)
            .bind(options.max_attempts)
            .fetch_all(&self.pg_pool)
            .await?;

        Ok(jobs)
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

        sqlx::query(&sql)
            .bind(worker_ids)
            .execute(&self.pg_pool)
            .await?;

        Ok(())
    }

    /// Runs database cleanup tasks to maintain performance.
    ///
    /// Executes the specified cleanup tasks, which helps keep
    /// the database size manageable and improves query performance.
    ///
    /// When `GcTaskIdentifiers` is included in the tasks and this `WorkerUtils` instance
    /// was created with task details (via `with_task_details`), the task details will be
    /// automatically refreshed after cleanup to ensure the worker can still pick up jobs.
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
        let has_gc_task_identifiers = tasks
            .iter()
            .any(|t| matches!(t, CleanupTask::GcTaskIdentifiers));

        if has_gc_task_identifiers {
            if let Some(task_details) = &self.task_details {
                let mut guard = task_details.write().await;
                let task_names = guard.task_names();

                for task in tasks {
                    task.execute(&self.pg_pool, &self.escaped_schema).await?;
                }

                let refreshed =
                    get_tasks_details(&self.pg_pool, &self.escaped_schema, task_names).await?;
                *guard = refreshed;
                return Ok(());
            }
        }

        for task in tasks {
            task.execute(&self.pg_pool, &self.escaped_schema).await?;
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
        migrate(&self.pg_pool, &self.escaped_schema).await?;
        Ok(())
    }
}
