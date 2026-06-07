use chrono::{DateTime, Utc};
use graphile_worker::Database;
use serde_json::Value;
use sqlx::{FromRow, PgPool};

#[derive(FromRow, Debug, PartialEq, Eq)]
pub struct JobWithQueueName {
    pub id: i64,
    pub job_queue_id: Option<i32>,
    pub task_identifier: String,
    pub queue_name: Option<String>,
    pub payload: serde_json::Value,
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
    pub task_id: i32,
    pub is_available: bool,
}

#[derive(FromRow, Debug)]
pub struct JobQueue {
    pub id: i32,
    pub queue_name: String,
    pub job_count: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
}

#[derive(FromRow, Debug)]
pub struct Migration {
    pub id: i32,
    pub ts: DateTime<Utc>,
    pub breaking: bool,
}

#[derive(Clone, Debug)]
pub struct TestDatabase {
    pub source_pool: PgPool,
    pub test_pool: PgPool,
    pub database: Database,
    pub name: String,
}
