use graphile_worker_database::Schema;

#[derive(Debug, Clone, Copy)]
pub(crate) enum PrivateTable {
    Jobs,
    JobQueues,
    Tasks,
    Workers,
}

impl PrivateTable {
    pub(crate) fn qualified(self, schema: &Schema) -> String {
        schema.private_table(self.as_str())
    }

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
    pub(crate) fn qualified(self, schema: &Schema) -> String {
        schema.function(self.as_str())
    }

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
