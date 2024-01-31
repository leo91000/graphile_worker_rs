use crate::errors::GraphileWorkerError;
use chrono::Utc;
use getset::Getters;
use sqlx::{query, PgExecutor};
use tracing::info;

/// Behavior when an existing job with the same job key is found is controlled by this setting
#[derive(Debug, Default)]
pub enum JobKeyMode {
    /// Overwrites the unlocked job with the new values. This is primarily useful for rescheduling, updating, or debouncing
    /// (delaying execution until there have been no events for at least a certain time period).
    /// Locked jobs will cause a new job to be scheduled instead.
    #[default]
    Replace,
    /// overwrites the unlocked job with the new values, but preserves run_at.
    /// This is primarily useful for throttling (executing at most once over a given time period).
    /// Locked jobs will cause a new job to be scheduled instead.
    PreserveRunAt,
    UnsafeDedupe,
}

impl JobKeyMode {
    /// Get the string representation of the job key mode
    /// This is used in the SQL query
    fn format(&self) -> &str {
        match self {
            JobKeyMode::Replace => "replace",
            JobKeyMode::PreserveRunAt => "preserve_run_at",
            JobKeyMode::UnsafeDedupe => "unsafe_dedupe",
        }
    }
}

/// Job options when adding a job to the queue
#[derive(Getters, Debug, Default)]
pub struct JobSpec {
    /// Add the job to a named queue so it executes serially with other jobs in the same queue.
    pub queue_name: Option<String>,
    /// Override the time at which the job should be run (instead of now).
    pub run_at: Option<chrono::DateTime<Utc>>,
    /// Override the max_attempts of the job (the max number of retries before giving up).
    pub max_attempts: Option<i16>,
    /// Replace/update the existing job with this key, if present.
    pub job_key: Option<String>,
    /// If jobKey is specified, affects what it does.
    pub job_key_mode: Option<JobKeyMode>,
    /// Override the priority of the job (affects the order in which it is executed).
    pub priority: Option<i16>,
    /// An optional text array representing a flags to attach to the job.
    /// Can be used alongside the forbiddenFlags option in library mode to implement complex rate limiting
    /// or other behaviors which requiring skipping jobs at runtime
    pub flags: Option<Vec<String>>,
}

/// Add a job to the queue
pub async fn add_job(
    executor: impl for<'e> PgExecutor<'e>,
    escaped_schema: &str,
    identifier: &str,
    payload: serde_json::Value,
    spec: JobSpec,
) -> Result<(), GraphileWorkerError> {
    let sql = format!(
        r#"
            select * from {escaped_schema}.add_job(
                identifier => $1::text,
                payload => $2::json,
                queue_name => $3::text,
                run_at => $4::timestamptz,
                max_attempts => $5::int,
                job_key => $6::text,
                priority => $7::int,
                flags => $8::text[],
                job_key_mode => $9::text
            );
        "#
    );

    let job_key_mode = spec.job_key_mode.map(|jkm| jkm.format().to_string());

    query(&sql)
        .bind(identifier)
        .bind(&payload)
        .bind(spec.queue_name)
        .bind(spec.run_at)
        .bind(spec.max_attempts)
        .bind(spec.job_key)
        .bind(spec.priority)
        .bind(spec.flags)
        .bind(job_key_mode)
        .execute(executor)
        .await?;

    info!(
        identifier,
        payload = ?payload,
        "Job added to queue"
    );

    Ok(())
}
