use chrono::{DateTime, Utc};
use getset::Getters;
use serde_json::Value;
use sqlx::FromRow;

#[derive(FromRow, Getters, Debug)]
#[getset(get = "pub")]
#[allow(dead_code)]
pub struct DbJob {
    id: i64,
    /// FK to tasks
    job_queue_id: Option<i32>,
    /// The JSON payload of the job
    payload: serde_json::Value,
    /// Lower number means it should run sooner
    priority: i16,
    /// When it was due to run
    run_at: DateTime<Utc>,
    /// How many times it has been attempted
    attempts: i16,
    /// The limit for the number of times it should be attempted
    max_attempts: i16,
    /// If attempts > 0, why did it fail last ?
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// "job_key" - unique identifier for easy update from user code
    key: Option<String>,
    /// A count of the revision numbers
    revision: i32,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    flags: Option<Value>,
    /// The task ID of the job
    task_id: i32,
}

#[derive(FromRow, Getters, Debug)]
#[getset(get = "pub")]
#[allow(dead_code)]
pub struct Job {
    id: i64,
    /// FK to tasks
    job_queue_id: Option<i32>,
    /// The JSON payload of the job
    payload: serde_json::Value,
    /// Lower number means it should run sooner
    priority: i16,
    /// When it was due to run
    run_at: DateTime<Utc>,
    /// How many times it has been attempted
    attempts: i16,
    /// The limit for the number of times it should be attempted
    max_attempts: i16,
    /// If attempts > 0, why did it fail last ?
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// "job_key" - unique identifier for easy update from user code
    key: Option<String>,
    /// A count of the revision numbers
    revision: i32,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    flags: Option<Value>,
    /// The task ID of the job
    task_id: i32,
    /// The task identifier of the job, shorcut to task.identifier
    task_identifier: String,
}

impl From<DbJob> for Job {
    fn from(db_job: DbJob) -> Job {
        Job {
            id: db_job.id,
            job_queue_id: db_job.job_queue_id,
            payload: db_job.payload,
            priority: db_job.priority,
            run_at: db_job.run_at,
            attempts: db_job.attempts,
            max_attempts: db_job.max_attempts,
            last_error: db_job.last_error,
            created_at: db_job.created_at,
            updated_at: db_job.updated_at,
            key: db_job.key,
            revision: db_job.revision,
            locked_at: db_job.locked_at,
            locked_by: db_job.locked_by,
            flags: db_job.flags,
            task_id: db_job.task_id,
            task_identifier: "".to_string(),
        }
    }
}

impl Job {
    pub fn from_db_job(db_job: DbJob, task_identifier: String) -> Job {
        Job {
            id: db_job.id,
            job_queue_id: db_job.job_queue_id,
            payload: db_job.payload,
            priority: db_job.priority,
            run_at: db_job.run_at,
            attempts: db_job.attempts,
            max_attempts: db_job.max_attempts,
            last_error: db_job.last_error,
            created_at: db_job.created_at,
            updated_at: db_job.updated_at,
            key: db_job.key,
            revision: db_job.revision,
            locked_at: db_job.locked_at,
            locked_by: db_job.locked_by,
            flags: db_job.flags,
            task_id: db_job.task_id,
            task_identifier,
        }
    }
}
