use chrono::{DateTime, Utc};
use getset::Getters;
use serde_json::Value;

/// Named field carrier for constructing a `DbJob`.
///
/// This avoids positional construction at database boundaries where adjacent
/// fields often have the same Rust type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbJobData {
    pub id: i64,
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
    pub task_id: i32,
}

/// `DbJob` represents a job as stored in the database.
///
/// It contains all the fields from the database table but doesn't include
/// the task identifier string, which is provided separately when constructing
/// a `Job` instance for use in the worker system.
#[cfg_attr(feature = "driver-sqlx", derive(sqlx::FromRow))]
#[derive(Getters, Debug, Clone, PartialEq, Eq)]
#[getset(get = "pub")]
#[allow(dead_code)]
pub struct DbJob {
    /// Unique identifier for this job
    pub(crate) id: i64,
    /// Foreign key to job_queues table if this job is part of a queue
    pub(crate) job_queue_id: Option<i32>,
    /// The JSON payload/data associated with this job
    pub(crate) payload: serde_json::Value,
    /// Priority level (lower number means higher priority)
    pub(crate) priority: i16,
    /// When the job is scheduled to run
    pub(crate) run_at: DateTime<Utc>,
    /// How many times this job has been attempted
    pub(crate) attempts: i16,
    /// Maximum number of retry attempts before considering the job permanently failed
    pub(crate) max_attempts: i16,
    /// Error message from the last failed attempt
    pub(crate) last_error: Option<String>,
    /// When the job was created
    pub(crate) created_at: DateTime<Utc>,
    /// When the job was last updated
    pub(crate) updated_at: DateTime<Utc>,
    /// Optional unique key for identifying/updating this job from user code
    pub(crate) key: Option<String>,
    /// Counter tracking the number of revisions/updates to this job
    pub(crate) revision: i32,
    /// When the job was locked for processing
    pub(crate) locked_at: Option<DateTime<Utc>>,
    /// Worker ID that has locked this job
    pub(crate) locked_by: Option<String>,
    /// Optional JSON flags to control job processing behavior
    pub(crate) flags: Option<Value>,
    /// Foreign key to the tasks table identifying the task type
    pub(crate) task_id: i32,
}

impl DbJob {
    pub fn from_data(data: DbJobData) -> Self {
        Self {
            id: data.id,
            job_queue_id: data.job_queue_id,
            payload: data.payload,
            priority: data.priority,
            run_at: data.run_at,
            attempts: data.attempts,
            max_attempts: data.max_attempts,
            last_error: data.last_error,
            created_at: data.created_at,
            updated_at: data.updated_at,
            key: data.key,
            revision: data.revision,
            locked_at: data.locked_at,
            locked_by: data.locked_by,
            flags: data.flags,
            task_id: data.task_id,
        }
    }

    pub fn into_data(self) -> DbJobData {
        DbJobData {
            id: self.id,
            job_queue_id: self.job_queue_id,
            payload: self.payload,
            priority: self.priority,
            run_at: self.run_at,
            attempts: self.attempts,
            max_attempts: self.max_attempts,
            last_error: self.last_error,
            created_at: self.created_at,
            updated_at: self.updated_at,
            key: self.key,
            revision: self.revision,
            locked_at: self.locked_at,
            locked_by: self.locked_by,
            flags: self.flags,
            task_id: self.task_id,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        job_queue_id: Option<i32>,
        payload: serde_json::Value,
        priority: i16,
        run_at: DateTime<Utc>,
        attempts: i16,
        max_attempts: i16,
        last_error: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        key: Option<String>,
        revision: i32,
        locked_at: Option<DateTime<Utc>>,
        locked_by: Option<String>,
        flags: Option<Value>,
        task_id: i32,
    ) -> Self {
        Self::from_data(DbJobData {
            id,
            job_queue_id,
            payload,
            priority,
            run_at,
            attempts,
            max_attempts,
            last_error,
            created_at,
            updated_at,
            key,
            revision,
            locked_at,
            locked_by,
            flags,
            task_id,
        })
    }
}
