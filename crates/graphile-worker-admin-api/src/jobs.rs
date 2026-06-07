use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::defaults::{default_limit, default_payload};

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
pub struct ListJobsResponse {
    pub jobs: Vec<ListedJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JobActionResponse {
    pub message: String,
    #[serde(default)]
    pub jobs: Vec<DbJobOutput>,
}
