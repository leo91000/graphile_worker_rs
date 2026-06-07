use chrono::{DateTime, Utc};
use derive_builder::Builder;
use getset::Getters;
use serde_json::Value;

use crate::{DbJob, DbJobData};

/// `Job` extends `DbJob` with an additional task_identifier field.
///
/// This struct is used throughout the worker system for job processing and
/// contains everything needed to execute a job, including the string identifier
/// of the task which is used to look up the appropriate handler function.
#[cfg_attr(feature = "driver-sqlx", derive(sqlx::FromRow))]
#[derive(Getters, Debug, Clone, PartialEq, Eq, Builder)]
#[getset(get = "pub")]
#[builder(build_fn(private, name = "build_internal"), pattern = "owned")]
#[allow(dead_code)]
pub struct Job {
    #[builder(default)]
    pub(crate) id: i64,
    #[builder(default, setter(strip_option))]
    pub(crate) job_queue_id: Option<i32>,
    #[builder(default = "serde_json::json!({})")]
    pub(crate) payload: serde_json::Value,
    #[builder(default)]
    pub(crate) priority: i16,
    #[builder(default = "Utc::now()")]
    pub(crate) run_at: DateTime<Utc>,
    #[builder(default)]
    pub(crate) attempts: i16,
    #[builder(default = "25")]
    pub(crate) max_attempts: i16,
    #[builder(default, setter(strip_option))]
    pub(crate) last_error: Option<String>,
    #[builder(default = "Utc::now()")]
    pub(crate) created_at: DateTime<Utc>,
    #[builder(default = "Utc::now()")]
    pub(crate) updated_at: DateTime<Utc>,
    #[builder(default, setter(strip_option))]
    pub(crate) key: Option<String>,
    #[builder(default)]
    pub(crate) revision: i32,
    #[builder(default, setter(strip_option))]
    pub(crate) locked_at: Option<DateTime<Utc>>,
    #[builder(default, setter(strip_option))]
    pub(crate) locked_by: Option<String>,
    #[builder(default, setter(strip_option))]
    pub(crate) flags: Option<Value>,
    #[builder(default)]
    pub(crate) task_id: i32,
    #[builder(default, setter(into))]
    pub(crate) task_identifier: String,
}

/// Conversion from `Job` to `DbJob`, dropping the task_identifier field
impl From<Job> for DbJob {
    fn from(job: Job) -> DbJob {
        DbJob::from_data(job.into_db_job_data())
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
    pub fn from_db_job(db_job: DbJob, task_identifier: String) -> Job {
        let db_job = db_job.into_data();
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

    fn into_db_job_data(self) -> DbJobData {
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
}

impl JobBuilder {
    /// Builds the Job with all configured values.
    pub fn build(self) -> Job {
        self.build_internal()
            .expect("All fields have defaults, build should never fail")
    }
}
