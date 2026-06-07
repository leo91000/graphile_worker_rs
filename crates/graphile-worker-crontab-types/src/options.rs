use getset::Getters;

use crate::CrontabFill;

/// Behavior when an existing job with the same job key is found is controlled by this setting
#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq, Clone)]
pub enum JobKeyMode {
    /// Overwrites the unlocked job with the new values. This is primarily useful for rescheduling, updating, or debouncing
    /// (delaying execution until there have been no events for at least a certain time period).
    /// Locked jobs will cause a new job to be scheduled instead.
    #[serde(rename = "replace")]
    Replace,
    /// overwrites the unlocked job with the new values, but preserves run_at.
    /// This is primarily useful for throttling (executing at most once over a given time period).
    /// Locked jobs will cause a new job to be scheduled instead.
    #[serde(rename = "preserve_run_at")]
    PreserveRunAt,
}

/// Crontab options
#[derive(Debug, PartialEq, Eq, Default, Getters, Clone)]
#[getset(get = "pub")]
pub struct CrontabOptions {
    /// The ID is a unique alphanumeric case-sensitive identifier starting with a letter
    /// Specify an identifier for this crontab entry;
    /// By default this will use the task identifier,
    /// but if you want more than one schedule for the same task (e.g. with different payload, or different times)
    /// then you will need to supply a unique identifier explicitly.
    pub id: Option<String>,
    /// Backfill any entries from the last time period,
    /// for example if the worker was not running
    /// when they were due to be executed (by default, no backfilling).
    pub fill: Option<CrontabFill>,
    /// Override the max_attempts of the job (the max number of retries before giving up).
    pub max: Option<u16>,
    /// Add the job to a named queue so it executes serially with other jobs in the same queue.
    pub queue: Option<String>,
    /// Override the priority of the job (affects the order in which it is executed).
    pub priority: Option<i16>,
    /// Replace/update the existing job with this key, if present.
    pub job_key: Option<String>,
    /// If jobKey is specified, affects what it does.
    pub job_key_mode: Option<JobKeyMode>,
}
