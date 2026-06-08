use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::GraphileWorkerError;
use graphile_worker_queries::schema_names::PrivateTable;

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
        schema: &Schema,
        task_identifiers_to_keep: &[String],
    ) -> Result<(), GraphileWorkerError> {
        match self {
            CleanupTask::DeletePermanentlyFailedJobs | CleanupTask::DeletePermenantlyFailedJobs => {
                let jobs = PrivateTable::Jobs.qualified(schema);
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
                let jobs = PrivateTable::Jobs.qualified(schema);
                let tasks = PrivateTable::Tasks.qualified(schema);
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
                let jobs = PrivateTable::Jobs.qualified(schema);
                let job_queues = PrivateTable::JobQueues.qualified(schema);
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
