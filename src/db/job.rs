use chrono::prelude::*;
use getset::Getters;
use sqlx::FromRow;

#[derive(FromRow, Getters)]
#[getset(get = "pub")]
pub struct Job {
    id: i32,
    job_queue_id: i32,
    task_id: i32,
    payload: Vec<u8>,
    priority: i32,
    run_at: DateTime<Utc>,
    max_attempts: i16,
    last_error: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    key: String,
    locked_at: DateTime<Utc>,
    locked_by: String,
    revision: i32,
    flags: serde_json::Value,
    is_available: bool,
}
