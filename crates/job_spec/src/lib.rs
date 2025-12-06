use std::fmt::{Display, Formatter};

use chrono::Utc;
use derive_builder::Builder;
use getset::{Getters, MutGetters, Setters};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum JobKeyMode {
    #[default]
    Replace,
    PreserveRunAt,
    UnsafeDedupe,
}

impl Display for JobKeyMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            JobKeyMode::Replace => "replace",
            JobKeyMode::PreserveRunAt => "preserve_run_at",
            JobKeyMode::UnsafeDedupe => "unsafe_dedupe",
        };
        write!(f, "{display}")
    }
}

#[derive(Getters, Setters, MutGetters, Debug, Default, Clone, Builder)]
#[getset(get = "pub", set = "pub", get_mut = "pub")]
#[builder(
    build_fn(private, name = "build_internal"),
    setter(strip_option),
    default,
    pattern = "owned"
)]
pub struct JobSpec {
    #[builder(setter(into))]
    pub queue_name: Option<String>,

    #[builder(setter(into))]
    pub run_at: Option<chrono::DateTime<Utc>>,

    pub max_attempts: Option<i16>,

    #[builder(setter(into))]
    pub job_key: Option<String>,

    #[builder(setter(into))]
    pub job_key_mode: Option<JobKeyMode>,

    pub priority: Option<i16>,

    #[builder(setter(into))]
    pub flags: Option<Vec<String>>,
}

impl JobSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> JobSpecBuilder {
        JobSpecBuilder::new()
    }
}

impl JobSpecBuilder {
    pub fn new() -> Self {
        Self::default()
    }

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
