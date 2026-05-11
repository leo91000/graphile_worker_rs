use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JobState {
    #[default]
    All,
    Ready,
    Scheduled,
    Locked,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct JobStats {
    pub(crate) total: i64,
    pub(crate) ready: i64,
    pub(crate) scheduled: i64,
    pub(crate) locked: i64,
    pub(crate) failed: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct QueueRow {
    pub(crate) id: i32,
    pub(crate) queue_name: String,
    pub(crate) locked_at: Option<DateTime<Utc>>,
    pub(crate) locked_by: Option<String>,
    pub(crate) job_count: i64,
    pub(crate) ready_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct LockedWorkerRow {
    pub(crate) worker_id: String,
    pub(crate) locked_jobs: i64,
    pub(crate) locked_queues: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AddJobRequest {
    pub(crate) identifier: String,
    #[serde(default)]
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct MessageResponse {
    pub(crate) message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AdminClientConfig {
    pub(crate) auth_mode: AuthMode,
    pub(crate) auth_header: String,
    pub(crate) csrf: String,
    pub(crate) csrf_header: String,
    pub(crate) read_only: bool,
    pub(crate) schema: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthMode {
    Basic,
    Bearer,
    Header,
    None,
}

impl AuthMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "basic" => Ok(Self::Basic),
            "bearer" => Ok(Self::Bearer),
            "header" => Ok(Self::Header),
            "none" => Ok(Self::None),
            value => Err(format!("unsupported auth mode `{value}`")),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Bearer => "bearer",
            Self::Header => "header",
            Self::None => "none",
        }
    }

    pub(crate) fn requires_token(self) -> bool {
        matches!(self, Self::Bearer | Self::Header)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Modal {
    AddJob,
    FailJobs,
    Reschedule,
    RemoveKey,
    JobDetails(ListedJob),
}
