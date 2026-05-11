use chrono::{DateTime, Utc};
use graphile_worker::worker_utils::CleanupTask;
use graphile_worker::{DbJob, Job, JobKeyMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::auth::AdminAuthSummary;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ListJobsParams {
    #[serde(default)]
    pub(crate) state: JobState,
    pub(crate) identifier: Option<String>,
    pub(crate) queue: Option<String>,
    pub(crate) search: Option<String>,
    #[serde(default = "default_limit")]
    pub(crate) limit: i64,
    #[serde(default)]
    pub(crate) offset: i64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JobState {
    #[default]
    All,
    Ready,
    Scheduled,
    Locked,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, sqlx::FromRow)]
pub(crate) struct ListedJob {
    pub(crate) id: i64,
    pub(crate) task_identifier: String,
    pub(crate) queue_name: Option<String>,
    pub(crate) payload: Value,
    pub(crate) priority: i16,
    pub(crate) run_at: DateTime<Utc>,
    pub(crate) attempts: i16,
    pub(crate) max_attempts: i16,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) key: Option<String>,
    pub(crate) locked_at: Option<DateTime<Utc>>,
    pub(crate) locked_by: Option<String>,
    pub(crate) revision: i32,
    pub(crate) flags: Option<Value>,
    pub(crate) is_available: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, sqlx::FromRow)]
pub(crate) struct JobStats {
    pub(crate) total: i64,
    pub(crate) ready: i64,
    pub(crate) scheduled: i64,
    pub(crate) locked: i64,
    pub(crate) failed: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::FromRow)]
pub(crate) struct QueueRow {
    pub(crate) id: i32,
    pub(crate) queue_name: String,
    pub(crate) locked_at: Option<DateTime<Utc>>,
    pub(crate) locked_by: Option<String>,
    pub(crate) job_count: i64,
    pub(crate) ready_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::FromRow)]
pub(crate) struct LockedWorkerRow {
    pub(crate) worker_id: String,
    pub(crate) locked_jobs: i64,
    pub(crate) locked_queues: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct DbJobOutput {
    pub(crate) id: i64,
    pub(crate) task_id: i32,
    pub(crate) task_identifier: Option<String>,
    pub(crate) job_queue_id: Option<i32>,
    pub(crate) payload: Value,
    pub(crate) priority: i16,
    pub(crate) run_at: DateTime<Utc>,
    pub(crate) attempts: i16,
    pub(crate) max_attempts: i16,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) key: Option<String>,
    pub(crate) revision: i32,
    pub(crate) locked_at: Option<DateTime<Utc>>,
    pub(crate) locked_by: Option<String>,
    pub(crate) flags: Option<Value>,
}

impl DbJobOutput {
    pub(crate) fn from_db_job(job: &DbJob, task_identifier: Option<String>) -> Self {
        Self {
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

    pub(crate) fn from_job(job: &Job) -> Self {
        Self {
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AddJobRequest {
    pub(crate) identifier: String,
    #[serde(default = "default_payload")]
    pub(crate) payload: Value,
    pub(crate) queue: Option<String>,
    pub(crate) run_at: Option<DateTime<Utc>>,
    pub(crate) max_attempts: Option<i16>,
    pub(crate) key: Option<String>,
    pub(crate) job_key_mode: Option<JobKeyModeRequest>,
    pub(crate) priority: Option<i16>,
    pub(crate) flags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JobKeyModeRequest {
    Replace,
    PreserveRunAt,
    UnsafeDedupe,
}

impl From<JobKeyModeRequest> for JobKeyMode {
    fn from(value: JobKeyModeRequest) -> Self {
        match value {
            JobKeyModeRequest::Replace => JobKeyMode::Replace,
            JobKeyModeRequest::PreserveRunAt => JobKeyMode::PreserveRunAt,
            JobKeyModeRequest::UnsafeDedupe => JobKeyMode::UnsafeDedupe,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct JobActionRequest {
    pub(crate) action: JobAction,
    pub(crate) ids: Vec<i64>,
    pub(crate) reason: Option<String>,
    pub(crate) run_at: Option<DateTime<Utc>>,
    pub(crate) priority: Option<i16>,
    pub(crate) attempts: Option<i16>,
    pub(crate) max_attempts: Option<i16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JobAction {
    Complete,
    Fail,
    RunNow,
    Reschedule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RemoveJobByKeyRequest {
    pub(crate) key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct MaintenanceRequest {
    pub(crate) action: MaintenanceAction,
    #[serde(default)]
    pub(crate) cleanup_tasks: Vec<CleanupTaskName>,
    #[serde(default)]
    pub(crate) worker_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MaintenanceAction {
    Migrate,
    Cleanup,
    ForceUnlock,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CleanupTaskName {
    DeletePermanentlyFailedJobs,
    GcTaskIdentifiers,
    GcJobQueues,
}

impl CleanupTaskName {
    pub(crate) fn all() -> Vec<Self> {
        vec![
            Self::DeletePermanentlyFailedJobs,
            Self::GcTaskIdentifiers,
            Self::GcJobQueues,
        ]
    }
}

impl From<CleanupTaskName> for CleanupTask {
    fn from(value: CleanupTaskName) -> Self {
        match value {
            CleanupTaskName::DeletePermanentlyFailedJobs => {
                CleanupTask::DeletePermenantlyFailedJobs
            }
            CleanupTaskName::GcTaskIdentifiers => CleanupTask::GcTaskIdentifiers,
            CleanupTaskName::GcJobQueues => CleanupTask::GcJobQueues,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SessionResponse {
    pub(crate) schema: String,
    pub(crate) read_only: bool,
    pub(crate) csrf_header: String,
    pub(crate) auth: AdminAuthSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct OverviewResponse {
    pub(crate) stats: JobStats,
    pub(crate) queues: Vec<QueueRow>,
    pub(crate) workers: Vec<LockedWorkerRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ListJobsResponse {
    pub(crate) jobs: Vec<ListedJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct JobActionResponse {
    pub(crate) message: String,
    pub(crate) jobs: Vec<DbJobOutput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct MessageResponse {
    pub(crate) message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: String,
}

pub(crate) fn default_limit() -> i64 {
    100
}

fn default_payload() -> Value {
    serde_json::json!({})
}
