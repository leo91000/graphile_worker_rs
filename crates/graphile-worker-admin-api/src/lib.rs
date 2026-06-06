use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "sqlx")]
pub mod queries;
#[cfg(feature = "sqlx")]
mod sql;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ListJobsParams {
    #[serde(default)]
    pub state: JobState,
    pub identifier: Option<String>,
    pub queue: Option<String>,
    pub search: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JobState {
    #[default]
    All,
    Ready,
    Scheduled,
    Locked,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct ListedJob {
    pub id: i64,
    pub task_identifier: String,
    pub queue_name: Option<String>,
    pub payload: Value,
    pub priority: i16,
    pub run_at: DateTime<Utc>,
    pub attempts: i16,
    pub max_attempts: i16,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub key: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub revision: i32,
    pub flags: Option<Value>,
    pub is_available: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct JobStats {
    pub total: i64,
    pub ready: i64,
    pub scheduled: i64,
    pub locked: i64,
    pub failed: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct QueueRow {
    pub id: i32,
    pub queue_name: String,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub job_count: i64,
    pub ready_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct LockedWorkerRow {
    pub worker_id: String,
    pub locked_jobs: i64,
    pub locked_queues: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    pub last_heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub metadata: Option<Value>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DbJobOutput {
    pub id: i64,
    pub task_id: i32,
    pub task_identifier: Option<String>,
    pub job_queue_id: Option<i32>,
    pub payload: Value,
    pub priority: i16,
    pub run_at: DateTime<Utc>,
    pub attempts: i16,
    pub max_attempts: i16,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub key: Option<String>,
    pub revision: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub flags: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddJobRequest {
    pub identifier: String,
    #[serde(default = "default_payload")]
    pub payload: Value,
    pub queue: Option<String>,
    pub run_at: Option<DateTime<Utc>>,
    pub max_attempts: Option<i16>,
    pub key: Option<String>,
    pub job_key_mode: Option<JobKeyModeRequest>,
    pub priority: Option<i16>,
    pub flags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobKeyModeRequest {
    Replace,
    PreserveRunAt,
    UnsafeDedupe,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JobActionRequest {
    pub action: JobAction,
    pub ids: Vec<i64>,
    pub reason: Option<String>,
    pub run_at: Option<DateTime<Utc>>,
    pub priority: Option<i16>,
    pub attempts: Option<i16>,
    pub max_attempts: Option<i16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobAction {
    Complete,
    Fail,
    RunNow,
    Reschedule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoveJobByKeyRequest {
    pub key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MaintenanceRequest {
    pub action: MaintenanceAction,
    #[serde(default)]
    pub cleanup_tasks: Vec<CleanupTaskName>,
    #[serde(default)]
    pub worker_ids: Vec<String>,
    #[serde(default)]
    pub dry_run: bool,
    pub sweep_threshold_secs: Option<u64>,
    pub recovery_delay_secs: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaintenanceAction {
    Migrate,
    Cleanup,
    ForceUnlock,
    SweepStaleWorkers,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupTaskName {
    DeletePermanentlyFailedJobs,
    GcTaskIdentifiers,
    GcJobQueues,
}

impl CleanupTaskName {
    pub fn all() -> Vec<Self> {
        vec![
            Self::DeletePermanentlyFailedJobs,
            Self::GcTaskIdentifiers,
            Self::GcJobQueues,
        ]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OverviewResponse {
    pub stats: JobStats,
    pub queues: Vec<QueueRow>,
    pub workers: Vec<LockedWorkerRow>,
    pub active_workers: Vec<ActiveWorkerRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ListJobsResponse {
    pub jobs: Vec<ListedJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JobActionResponse {
    pub message: String,
    #[serde(default)]
    pub jobs: Vec<DbJobOutput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn default_limit() -> i64 {
    100
}

fn default_payload() -> Value {
    serde_json::json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_action_response_defaults_missing_jobs() {
        let response: JobActionResponse =
            serde_json::from_str(r#"{"message":"Completed 1 job(s)"}"#).unwrap();

        assert_eq!(response.message, "Completed 1 job(s)");
        assert!(response.jobs.is_empty());
    }

    #[test]
    fn add_job_request_defaults_payload_to_empty_object() {
        let request: AddJobRequest = serde_json::from_str(r#"{"identifier":"send_email"}"#)
            .expect("minimal add job request should deserialize");

        assert_eq!(request.identifier, "send_email");
        assert_eq!(request.payload, serde_json::json!({}));
    }
}
