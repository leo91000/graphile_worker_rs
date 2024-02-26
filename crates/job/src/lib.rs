use chrono::{DateTime, Utc};
use getset::Getters;
use serde_json::Value;
use sqlx::FromRow;

#[derive(FromRow, Getters, Debug, Clone, PartialEq, Eq)]
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

#[derive(FromRow, Getters, Debug, Clone, PartialEq, Eq)]
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

impl From<Job> for DbJob {
    fn from(job: Job) -> DbJob {
        DbJob {
            id: job.id,
            job_queue_id: job.job_queue_id,
            payload: job.payload,
            priority: job.priority,
            run_at: job.run_at,
            attempts: job.attempts,
            max_attempts: job.max_attempts,
            last_error: job.last_error,
            created_at: job.created_at,
            updated_at: job.updated_at,
            key: job.key,
            revision: job.revision,
            locked_at: job.locked_at,
            locked_by: job.locked_by,
            flags: job.flags,
            task_id: job.task_id,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_db_job() {
        let db_job = DbJob {
            id: 1,
            job_queue_id: Some(1),
            payload: serde_json::json!({}),
            priority: 1,
            run_at: Utc::now(),
            attempts: 1,
            max_attempts: 1,
            last_error: Some("error".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            key: Some("key".to_string()),
            revision: 1,
            locked_at: Some(Utc::now()),
            locked_by: Some("locked_by".to_string()),
            flags: Some(serde_json::json!({})),
            task_id: 1,
        };
        let task_identifier = "task_identifier".to_string();
        let job = Job::from_db_job(db_job, task_identifier);
        assert_eq!(job.id, 1);
        assert_eq!(job.job_queue_id, Some(1));
        assert_eq!(job.payload, serde_json::json!({}));
        assert_eq!(job.priority, 1);
        assert_eq!(job.attempts, 1);
        assert_eq!(job.max_attempts, 1);
        assert_eq!(job.last_error, Some("error".to_string()));
        assert_eq!(job.key, Some("key".to_string()));
        assert_eq!(job.revision, 1);
        assert_eq!(job.locked_by, Some("locked_by".to_string()));
        assert_eq!(job.task_id, 1);
        assert_eq!(job.task_identifier, "task_identifier".to_string());
    }

    #[test]
    fn test_from() {
        let job = Job {
            id: 1,
            job_queue_id: Some(1),
            payload: serde_json::json!({}),
            priority: 1,
            run_at: Utc::now(),
            attempts: 1,
            max_attempts: 1,
            last_error: Some("error".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            key: Some("key".to_string()),
            revision: 1,
            locked_at: Some(Utc::now()),
            locked_by: Some("locked_by".to_string()),
            flags: Some(serde_json::json!({})),
            task_id: 1,
            task_identifier: "task_identifier".to_string(),
        };

        let db_job: DbJob = job.clone().into();

        assert_eq!(db_job.id, 1);
        assert_eq!(db_job.job_queue_id, Some(1));
        assert_eq!(db_job.payload, serde_json::json!({}));
        assert_eq!(db_job.priority, 1);
        assert_eq!(db_job.attempts, 1);
        assert_eq!(db_job.max_attempts, 1);
        assert_eq!(db_job.last_error, Some("error".to_string()));
        assert_eq!(db_job.key, Some("key".to_string()));
        assert_eq!(db_job.revision, 1);
        assert_eq!(db_job.locked_by, Some("locked_by".to_string()));
        assert_eq!(db_job.task_id, 1);
    }
}
