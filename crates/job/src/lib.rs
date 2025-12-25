use chrono::{DateTime, Utc};
use derive_builder::Builder;
use getset::Getters;
use serde_json::Value;
use sqlx::FromRow;

/// `DbJob` represents a job as stored in the database.
///
/// It contains all the fields from the database table but doesn't include
/// the task identifier string, which is provided separately when constructing
/// a `Job` instance for use in the worker system.
#[derive(FromRow, Getters, Debug, Clone, PartialEq, Eq)]
#[getset(get = "pub")]
#[allow(dead_code)]
pub struct DbJob {
    /// Unique identifier for this job
    id: i64,
    /// Foreign key to job_queues table if this job is part of a queue
    job_queue_id: Option<i32>,
    /// The JSON payload/data associated with this job
    payload: serde_json::Value,
    /// Priority level (lower number means higher priority)
    priority: i16,
    /// When the job is scheduled to run
    run_at: DateTime<Utc>,
    /// How many times this job has been attempted
    attempts: i16,
    /// Maximum number of retry attempts before considering the job permanently failed
    max_attempts: i16,
    /// Error message from the last failed attempt
    last_error: Option<String>,
    /// When the job was created
    created_at: DateTime<Utc>,
    /// When the job was last updated
    updated_at: DateTime<Utc>,
    /// Optional unique key for identifying/updating this job from user code
    key: Option<String>,
    /// Counter tracking the number of revisions/updates to this job
    revision: i32,
    /// When the job was locked for processing
    locked_at: Option<DateTime<Utc>>,
    /// Worker ID that has locked this job
    locked_by: Option<String>,
    /// Optional JSON flags to control job processing behavior
    flags: Option<Value>,
    /// Foreign key to the tasks table identifying the task type
    task_id: i32,
}

/// `Job` extends `DbJob` with an additional task_identifier field.
///
/// This struct is used throughout the worker system for job processing and
/// contains everything needed to execute a job, including the string identifier
/// of the task which is used to look up the appropriate handler function.
#[derive(FromRow, Getters, Debug, Clone, PartialEq, Eq, Builder)]
#[getset(get = "pub")]
#[builder(build_fn(private, name = "build_internal"), pattern = "owned")]
#[allow(dead_code)]
pub struct Job {
    #[builder(default)]
    id: i64,
    #[builder(default, setter(strip_option))]
    job_queue_id: Option<i32>,
    #[builder(default = "serde_json::json!({})")]
    payload: serde_json::Value,
    #[builder(default)]
    priority: i16,
    #[builder(default = "Utc::now()")]
    run_at: DateTime<Utc>,
    #[builder(default)]
    attempts: i16,
    #[builder(default = "25")]
    max_attempts: i16,
    #[builder(default, setter(strip_option))]
    last_error: Option<String>,
    #[builder(default = "Utc::now()")]
    created_at: DateTime<Utc>,
    #[builder(default = "Utc::now()")]
    updated_at: DateTime<Utc>,
    #[builder(default, setter(strip_option))]
    key: Option<String>,
    #[builder(default)]
    revision: i32,
    #[builder(default, setter(strip_option))]
    locked_at: Option<DateTime<Utc>>,
    #[builder(default, setter(strip_option))]
    locked_by: Option<String>,
    #[builder(default, setter(strip_option))]
    flags: Option<Value>,
    #[builder(default)]
    task_id: i32,
    #[builder(default, setter(into))]
    task_identifier: String,
}

/// Conversion from `Job` to `DbJob`, dropping the task_identifier field
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
    /// Creates a new builder for constructing a `Job`.
    pub fn builder() -> JobBuilder {
        JobBuilder::default()
    }

    /// Creates a `Job` from a `DbJob` and a task identifier string.
    ///
    /// The task identifier is used to look up the appropriate handler function
    /// when the job is executed.
    ///
    /// # Arguments
    ///
    /// * `db_job` - The database job record
    /// * `task_identifier` - The string identifier for the task type
    ///
    /// # Returns
    ///
    /// A new `Job` instance with all fields from `db_job` plus the provided task identifier
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

impl JobBuilder {
    /// Builds the Job with all configured values.
    pub fn build(self) -> Job {
        self.build_internal()
            .expect("All fields have defaults, build should never fail")
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
