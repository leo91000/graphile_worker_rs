#![allow(dead_code)]

use chrono::Utc;
use graphile_worker::{JobKeyMode, JobSpec, JobSpecBuilder};

mod helpers;

#[tokio::test]
async fn test_job_spec_builder() {
    // Test the JobSpecBuilder
    let job_spec = JobSpecBuilder::new()
        .queue_name("test_queue")
        .run_at(Utc::now())
        .max_attempts(5)
        .job_key("test_key")
        .job_key_mode(JobKeyMode::PreserveRunAt)
        .priority(3)
        .flags(vec!["flag1".to_string(), "flag2".to_string()])
        .build();

    assert_eq!(*job_spec.queue_name(), Some("test_queue".to_string()));
    assert!(job_spec.run_at().is_some());
    assert_eq!(*job_spec.max_attempts(), Some(5));
    assert_eq!(*job_spec.job_key(), Some("test_key".to_string()));
    assert_eq!(job_spec.job_key_mode(), &Some(JobKeyMode::PreserveRunAt));
    assert_eq!(*job_spec.priority(), Some(3));

    let expected_flags = Some(vec!["flag1".to_string(), "flag2".to_string()]);
    assert_eq!(job_spec.flags(), &expected_flags);
}

#[tokio::test]
async fn test_job_spec_default() {
    // Test the default JobSpec
    let job_spec = JobSpec::default();

    assert_eq!(*job_spec.queue_name(), None);
    assert_eq!(*job_spec.run_at(), None);
    assert_eq!(*job_spec.max_attempts(), None);
    assert_eq!(*job_spec.job_key(), None);
    assert_eq!(*job_spec.job_key_mode(), None);
    assert_eq!(*job_spec.priority(), None);
    assert_eq!(*job_spec.flags(), None);
}

#[tokio::test]
async fn test_job_key_mode_to_string() {
    // Test string conversion
    assert_eq!(JobKeyMode::Replace.to_string(), "replace");
    assert_eq!(JobKeyMode::PreserveRunAt.to_string(), "preserve_run_at");
    assert_eq!(JobKeyMode::UnsafeDedupe.to_string(), "unsafe_dedupe");
}

#[tokio::test]
async fn test_job_spec_from_option() {
    // Test JobSpec::from with Some value
    let some_spec = Some(JobSpec::builder().queue_name("test_queue").build());
    let job_spec = JobSpec::from(some_spec);
    assert_eq!(*job_spec.queue_name(), Some("test_queue".to_string()));

    // Test JobSpec::from with None value
    let none_spec: Option<JobSpec> = None;
    let job_spec = JobSpec::from(none_spec);
    assert_eq!(*job_spec.queue_name(), None);
}
