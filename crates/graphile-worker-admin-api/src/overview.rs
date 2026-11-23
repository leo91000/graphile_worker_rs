use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

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
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub locked_at: Option<OffsetDateTime>,
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
    #[serde(with = "time::serde::rfc3339")]
    pub last_heartbeat_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    pub metadata: Option<Value>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OverviewResponse {
    pub stats: JobStats,
    pub queues: Vec<QueueRow>,
    pub workers: Vec<LockedWorkerRow>,
    pub active_workers: Vec<ActiveWorkerRow>,
}
