use std::fmt::{Display, Formatter};

use chrono::Utc;
use derive_builder::Builder;
use getset::{Getters, MutGetters, Setters};

/// Behavior when an existing job with the same job key is found is controlled by this setting
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

impl Display for JobKeyMode {
    /// Get the string representation of the job key mode
    /// This is used in the SQL query
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            JobKeyMode::Replace => "replace",
            JobKeyMode::PreserveRunAt => "preserve_run_at",
            JobKeyMode::UnsafeDedupe => "unsafe_dedupe",
        };
        write!(f, "{display}")
    }
}

/// Job options when adding a job to the queue
#[derive(Getters, Setters, MutGetters, Debug, Default, Clone, Builder)]
#[getset(get = "pub", set = "pub", get_mut = "pub")]
#[builder(
    build_fn(private, name = "build_internal"),
    setter(strip_option),
    default,
    pattern = "owned"
)]
pub struct JobSpec {
    /// Add the job to a named queue so it executes serially with other jobs in the same queue.
    #[builder(setter(into))]
    pub queue_name: Option<String>,
    /// Override the time at which the job should be run (instead of now).
    #[builder(setter(into))]
    pub run_at: Option<chrono::DateTime<Utc>>,
    /// Override the max_attempts of the job (the max number of retries before giving up).
    pub max_attempts: Option<i16>,
    /// Replace/update the existing job with this key, if present.
    #[builder(setter(into))]
    pub job_key: Option<String>,
    /// If jobKey is specified, affects what it does.
    #[builder(setter(into))]
    pub job_key_mode: Option<JobKeyMode>,
    /// Override the priority of the job (affects the order in which it is executed).
    pub priority: Option<i16>,
    /// An optional text array representing a flags to attach to the job.
    /// Can be used alongside the forbiddenFlags option in library mode to implement complex rate limiting
    /// or other behaviors which requiring skipping jobs at runtime
    #[builder(setter(into))]
    pub flags: Option<Vec<String>>,
}

impl JobSpec {
    /// Create a new instance of JobSpec
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for JobSpec
    pub fn builder() -> JobSpecBuilder {
        JobSpecBuilder::new()
    }
}

impl JobSpecBuilder {
    /// Create a new instance of JobSpecBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the JobSpec
    pub fn build(self) -> JobSpec {
        self.build_internal()
            .expect("There is a default value for all fields")
    }
}

impl From<Option<JobSpec>> for JobSpec {
    fn from(spec: Option<JobSpec>) -> Self {
        spec.unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_spec() {
        let now = Utc::now();
        let job_spec = JobSpecBuilder::new()
            .queue_name("default")
            .run_at(now.clone())
            .max_attempts(3)
            .job_key("job_key")
            .job_key_mode(JobKeyMode::Replace)
            .priority(1)
            .flags(vec!["flag".to_string()])
            .build();

        assert_eq!(job_spec.queue_name(), &Some("default".to_string()));
        assert_eq!(job_spec.run_at(), &Some(now));
        assert_eq!(job_spec.max_attempts(), &Some(3));
        assert_eq!(job_spec.job_key(), &Some("job_key".to_string()));
        assert_eq!(job_spec.job_key_mode(), &Some(JobKeyMode::Replace));
        assert_eq!(job_spec.priority(), &Some(1));
        assert_eq!(job_spec.flags(), &Some(vec!["flag".to_string()]));
    }

    #[test]
    fn should_build_unset_job_spec_without_panic() {
        let _ = JobSpecBuilder::new().build();
    }
}
