use std::fmt::{Display, Formatter};

use chrono::Utc;
use derive_builder::Builder;
use getset::{Getters, MutGetters, Setters};

/// Controls how jobs with the same key are handled when adding a new job.
///
/// When adding a job with a job_key that matches an existing job, this enum
/// determines what happens to the existing job.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum JobKeyMode {
    /// Overwrites the unlocked job with the new values.
    ///
    /// This is primarily useful for:
    /// - Rescheduling: Update a job's parameters without creating a duplicate
    /// - Updating: Change a job's payload or other properties
    /// - Debouncing: Delay execution until there have been no events for at least a certain time period
    ///
    /// If the existing job is currently locked (being processed), a new job will be scheduled instead.
    #[default]
    Replace,

    /// Overwrites the unlocked job with the new values, but preserves the original run_at timestamp.
    ///
    /// This is primarily useful for throttling (executing at most once over a given time period).
    /// If you have a job scheduled for the future and add another job with the same key using
    /// this mode, the job will keep its original scheduled time but update other parameters.
    ///
    /// If the existing job is currently locked (being processed), a new job will be scheduled instead.
    PreserveRunAt,

    /// Completely deduplicates jobs, even if they're currently being processed.
    ///
    /// CAUTION: This mode is potentially unsafe because it can create race conditions.
    /// If a job is already running and another job with the same key is added using this mode,
    /// the new job will be discarded, potentially losing data.
    ///
    /// Only use this when you're absolutely certain that duplicate jobs should be completely
    /// eliminated, regardless of their processing state.
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

/// Configuration options for a job being added to the queue.
///
/// JobSpec allows you to control various aspects of how and when a job is executed,
/// including scheduling, prioritization, retry behavior, and serialization via queues.
///
/// To create a JobSpec with fluent syntax, use the JobSpecBuilder:
///
/// ```
/// use graphile_worker::{JobSpecBuilder, JobKeyMode};
/// use chrono::Utc;
///
/// let spec = JobSpecBuilder::new()
///     .queue_name("emails")
///     .run_at(Utc::now() + chrono::Duration::minutes(5))
///     .priority(10)
///     .max_attempts(3)
///     .job_key("send_welcome_123")
///     .job_key_mode(JobKeyMode::Replace)
///     .build();
/// ```
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
    ///
    /// Jobs in the same queue will execute one after another, ensuring that operations on the same
    /// resource happen in sequence. For example, if you want operations on a user's account to happen
    /// in order, you might use a queue name like `user:123`.
    ///
    /// If not specified, the job will run in parallel with other jobs, limited only by the worker's
    /// concurrency setting.
    #[builder(setter(into))]
    pub queue_name: Option<String>,

    /// Override the time at which the job should be run (instead of now).
    ///
    /// If specified, the job will not be executed until this time is reached. This is useful for:
    /// - Scheduling future tasks (e.g., sending a reminder email)
    /// - Implementing retry delays
    /// - Rate limiting by scheduling jobs with increasing delays
    ///
    /// If not specified, the job will be eligible for execution immediately.
    #[builder(setter(into))]
    pub run_at: Option<chrono::DateTime<Utc>>,

    /// Override the maximum number of retry attempts before the job is considered permanently failed.
    ///
    /// When a job fails, it is automatically retried with an exponential backoff delay. This setting
    /// controls how many times to retry before giving up.
    ///
    /// The default is 25 attempts, which spans about 3 days of retry attempts.
    pub max_attempts: Option<i16>,

    /// Assign a unique key to the job for deduplication and updating.
    ///
    /// If a job with the same key already exists, the behavior is controlled by the `job_key_mode`.
    /// This can be used to:
    /// - Prevent duplicate jobs
    /// - Update existing jobs
    /// - Implement debouncing and throttling patterns
    ///
    /// See JobKeyMode for details on how jobs with the same key are handled.
    #[builder(setter(into))]
    pub job_key: Option<String>,

    /// Controls how existing jobs with the same job_key are handled.
    ///
    /// This only has an effect if job_key is specified.
    /// See the JobKeyMode enum for details on the available modes.
    #[builder(setter(into))]
    pub job_key_mode: Option<JobKeyMode>,

    /// Set the priority of the job (affects the order in which jobs are executed).
    ///
    /// Jobs with higher priority values are executed before jobs with lower priority.
    /// The default priority is 0.
    pub priority: Option<i16>,

    /// Attach flags to the job for filtering or specialized behavior.
    ///
    /// Flags can be used alongside the `forbidden_flags` option when configuring a worker
    /// to implement complex rate limiting or skip certain jobs at runtime.
    ///
    /// For example, you might add flags like "high_memory" or "network_intensive" and configure
    /// certain workers to avoid jobs with these flags.
    #[builder(setter(into))]
    pub flags: Option<Vec<String>>,
}

impl JobSpec {
    /// Creates a new instance of JobSpec with default values.
    ///
    /// Equivalent to calling `JobSpec::default()`.
    ///
    /// # Returns
    /// A new JobSpec instance with all fields set to their default values (None).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for constructing a JobSpec with a fluent API.
    ///
    /// The builder pattern allows more readable code when setting multiple
    /// fields on a JobSpec.
    ///
    /// # Returns
    /// A new JobSpecBuilder instance.
    ///
    /// # Example
    /// ```
    /// use graphile_worker::JobSpec;
    ///
    /// let spec = JobSpec::builder()
    ///     .queue_name("emails")
    ///     .priority(10)
    ///     .build();
    /// ```
    pub fn builder() -> JobSpecBuilder {
        JobSpecBuilder::new()
    }
}

impl JobSpecBuilder {
    /// Creates a new instance of JobSpecBuilder with default values.
    ///
    /// # Returns
    /// A new JobSpecBuilder instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the JobSpec from the current builder state.
    ///
    /// Finalizes the construction of a JobSpec using the values
    /// that have been set on the builder.
    ///
    /// # Returns
    /// A fully constructed JobSpec instance.
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
            .run_at(now)
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
