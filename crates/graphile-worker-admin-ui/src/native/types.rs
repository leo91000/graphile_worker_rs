use graphile_worker::worker_utils::CleanupTask;
use graphile_worker::{DbJob, Job, JobKeyMode};
use serde::Serialize;

use super::auth::AdminAuthSummary;

pub(crate) use graphile_worker_admin_api::{
    ActiveWorkerRow, AddJobRequest, CleanupTaskName, DbJobOutput, ErrorResponse, JobAction,
    JobActionRequest, JobActionResponse, JobKeyModeRequest, ListJobsParams, ListJobsResponse,
    ListedJob, MaintenanceAction, MaintenanceRequest, MessageResponse, OverviewResponse,
    RemoveJobByKeyRequest,
};

#[derive(Debug, Serialize)]
pub(crate) struct SessionResponse {
    pub(crate) schema: String,
    pub(crate) read_only: bool,
    pub(crate) csrf_header: String,
    pub(crate) auth: AdminAuthSummary,
}

pub(crate) fn job_key_mode_from_request(value: JobKeyModeRequest) -> JobKeyMode {
    match value {
        JobKeyModeRequest::Replace => JobKeyMode::Replace,
        JobKeyModeRequest::PreserveRunAt => JobKeyMode::PreserveRunAt,
        JobKeyModeRequest::UnsafeDedupe => JobKeyMode::UnsafeDedupe,
    }
}

pub(crate) fn cleanup_task_from_name(value: CleanupTaskName) -> CleanupTask {
    match value {
        CleanupTaskName::DeletePermanentlyFailedJobs => CleanupTask::DeletePermanentlyFailedJobs,
        CleanupTaskName::GcTaskIdentifiers => CleanupTask::GcTaskIdentifiers,
        CleanupTaskName::GcJobQueues => CleanupTask::GcJobQueues,
    }
}

pub(crate) fn db_job_output_from_db_job(
    job: &DbJob,
    task_identifier: Option<String>,
) -> DbJobOutput {
    DbJobOutput {
        id: *job.id(),
        task_id: *job.task_id(),
        task_identifier,
        job_queue_id: *job.job_queue_id(),
        payload: job.payload().clone(),
        priority: *job.priority(),
        run_at: *job.run_at(),
        attempts: *job.attempts(),
        max_attempts: *job.max_attempts(),
        last_error: job.last_error().clone(),
        created_at: *job.created_at(),
        updated_at: *job.updated_at(),
        key: job.key().clone(),
        revision: *job.revision(),
        locked_at: *job.locked_at(),
        locked_by: job.locked_by().clone(),
        flags: job.flags().clone(),
    }
}

pub(crate) fn db_job_output_from_job(job: &Job) -> DbJobOutput {
    DbJobOutput {
        id: *job.id(),
        task_id: *job.task_id(),
        task_identifier: Some(job.task_identifier().clone()),
        job_queue_id: *job.job_queue_id(),
        payload: job.payload().clone(),
        priority: *job.priority(),
        run_at: *job.run_at(),
        attempts: *job.attempts(),
        max_attempts: *job.max_attempts(),
        last_error: job.last_error().clone(),
        created_at: *job.created_at(),
        updated_at: *job.updated_at(),
        key: job.key().clone(),
        revision: *job.revision(),
        locked_at: *job.locked_at(),
        locked_by: job.locked_by().clone(),
        flags: job.flags().clone(),
    }
}
