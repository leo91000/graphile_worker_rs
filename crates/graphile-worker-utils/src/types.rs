use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;

use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_queries::schema_names::PrivateTable;

/// Types of database cleanup tasks that can be performed on the Graphile Worker schema.
///
/// These tasks help maintain database performance by removing unused records and
/// reducing database size over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupTask {
    /// Removes task identifier records that are no longer referenced by any jobs.
    /// This helps keep the `_private_tasks` table clean and smaller.
    /// Cleanup serializes with job insertion at READ COMMITTED isolation, and
    /// returns an error if it cannot acquire the task table lock within one second.
    /// While held, the lock pauses task registration and job insertion. The
    /// timeout bounds lock acquisition, not the duration of the cleanup query.
    /// Caller-owned transactions adding jobs should use READ COMMITTED when
    /// task cleanup may run concurrently; older snapshots at REPEATABLE READ
    /// or SERIALIZABLE can still refer to a task removed by cleanup.
    ///
    /// **Note**: When using `WorkerUtils::cleanup()` from a worker, task identifiers
    /// that the worker knows about will be preserved to support horizontal scaling.
    /// Standalone cleanup with an empty keep list can invalidate task IDs cached
    /// by running workers; stop those workers before collecting their task rows.
    GcTaskIdentifiers,

    /// Removes unlocked job queue records that are no longer referenced by any jobs.
    /// This helps keep the `_private_job_queues` table clean and smaller.
    /// Cleanup serializes with job insertion at READ COMMITTED isolation, and
    /// returns an error if it cannot acquire the queue table lock within one second.
    /// Waiting for or holding this lock can pause job insertion, claims and
    /// completion of queued jobs. The timeout bounds acquisition, not deletion.
    /// Caller-owned transactions adding jobs should use READ COMMITTED when
    /// queue cleanup may run concurrently; older snapshots at REPEATABLE READ
    /// or SERIALIZABLE can still refer to a queue removed by cleanup.
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
                // Deduplicate references in this statement so jobs are scanned
                // once, even when a plain anti-join would rescan them per task.
                // Its snapshot must be taken after the cleanup table lock.
                let sql = formatdoc!(
                    r#"
                        with used_tasks as materialized (
                            select distinct jobs.task_id from {jobs} jobs
                        )
                        delete from {tasks} tasks
                        where not exists (
                            select 1 from used_tasks
                            where used_tasks.task_id = tasks.id
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
                // Claims and queued completions wait for this table lock. Scan
                // jobs once and compare queues with the distinct referenced IDs,
                // rather than repeatedly scanning jobs while holding the lock.
                let sql = formatdoc!(
                    r#"
                        with used_queues as materialized (
                            select distinct jobs.job_queue_id from {jobs} jobs
                            where jobs.job_queue_id is not null
                        )
                        delete from {job_queues} job_queues
                        where job_queues.locked_at is null
                        and not exists (
                            select 1 from used_queues
                            where used_queues.job_queue_id = job_queues.id
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

    /// The job's priority. Lower numbers indicate higher priority and run sooner.
    /// Default priority is 0.
    pub priority: Option<i16>,

    /// How many times the job has been attempted.
    /// Normally this should not be manually set.
    pub attempts: Option<i16>,

    /// Maximum number of retry attempts before the job is considered permanently failed.
    /// Default is 25 attempts.
    pub max_attempts: Option<i16>,
}
