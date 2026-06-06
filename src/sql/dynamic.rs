use graphile_worker_database::{DbRow, FromDbCell};

use crate::errors::Result;

#[derive(Debug, Clone, Copy)]
pub(crate) struct DynamicSchema<'a> {
    escaped_schema: &'a str,
}

impl<'a> DynamicSchema<'a> {
    pub(crate) fn new(escaped_schema: &'a str) -> Self {
        Self { escaped_schema }
    }

    pub(crate) fn private_table(self, table: PrivateTable) -> String {
        format!("{}._private_{}", self.escaped_schema, table.as_str())
    }

    pub(crate) fn function(self, function: WorkerFunction) -> String {
        format!("{}.{}", self.escaped_schema, function.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PrivateTable {
    Jobs,
    JobQueues,
    Tasks,
    Workers,
}

impl PrivateTable {
    fn as_str(self) -> &'static str {
        match self {
            Self::Jobs => "jobs",
            Self::JobQueues => "job_queues",
            Self::Tasks => "tasks",
            Self::Workers => "workers",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WorkerFunction {
    AddJob,
    AddJobs,
    CompleteJobs,
    DeleteStaleWorkers,
    ForceUnlockWorkers,
    ListOrphanLockedWorkers,
    ListStaleWorkers,
    PermanentlyFailJobs,
    RecoverDeadWorkerJobs,
    RemoveJob,
    RescheduleJobs,
    WorkerDeregister,
    WorkerHeartbeat,
}

impl WorkerFunction {
    fn as_str(self) -> &'static str {
        match self {
            Self::AddJob => "add_job",
            Self::AddJobs => "add_jobs",
            Self::CompleteJobs => "complete_jobs",
            Self::DeleteStaleWorkers => "delete_stale_workers",
            Self::ForceUnlockWorkers => "force_unlock_workers",
            Self::ListOrphanLockedWorkers => "list_orphan_locked_workers",
            Self::ListStaleWorkers => "list_stale_workers",
            Self::PermanentlyFailJobs => "permanently_fail_jobs",
            Self::RecoverDeadWorkerJobs => "recover_dead_worker_jobs",
            Self::RemoveJob => "remove_job",
            Self::RescheduleJobs => "reschedule_jobs",
            Self::WorkerDeregister => "worker_deregister",
            Self::WorkerHeartbeat => "worker_heartbeat",
        }
    }
}

pub(crate) fn collect_column<T>(rows: &[DbRow], column: &str) -> Result<Vec<T>>
where
    T: FromDbCell,
{
    rows.iter()
        .map(|row| row.try_get::<T>(column).map_err(Into::into))
        .collect()
}

pub(crate) fn get_required<T>(row: &DbRow, column: &str) -> Result<T>
where
    T: FromDbCell,
{
    row.try_get(column).map_err(Into::into)
}
