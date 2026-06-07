use chrono::Utc;

use crate::{DbJob, DbJobData, Job};

#[test]
fn test_from_db_job() {
    let db_job = DbJob::from_data(DbJobData {
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
    });
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

#[test]
fn db_job_new_and_into_data_preserve_all_fields() {
    let now = Utc::now();
    let job = DbJob::new(
        7,
        Some(2),
        serde_json::json!({ "hello": "world" }),
        3,
        now,
        4,
        5,
        Some("last error".to_string()),
        now,
        now,
        Some("job-key".to_string()),
        6,
        Some(now),
        Some("worker".to_string()),
        Some(serde_json::json!({ "flag": true })),
        8,
    );

    assert_eq!(*job.id(), 7);
    assert_eq!(*job.job_queue_id(), Some(2));
    assert_eq!(job.payload(), &serde_json::json!({ "hello": "world" }));
    assert_eq!(*job.priority(), 3);
    assert_eq!(*job.run_at(), now);
    assert_eq!(*job.attempts(), 4);
    assert_eq!(*job.max_attempts(), 5);
    assert_eq!(job.last_error().as_deref(), Some("last error"));
    assert_eq!(*job.created_at(), now);
    assert_eq!(*job.updated_at(), now);
    assert_eq!(job.key().as_deref(), Some("job-key"));
    assert_eq!(*job.revision(), 6);
    assert_eq!(*job.locked_at(), Some(now));
    assert_eq!(job.locked_by().as_deref(), Some("worker"));
    assert_eq!(job.flags(), &Some(serde_json::json!({ "flag": true })));
    assert_eq!(*job.task_id(), 8);

    let data = job.into_data();
    assert_eq!(data.id, 7);
    assert_eq!(data.job_queue_id, Some(2));
    assert_eq!(data.payload, serde_json::json!({ "hello": "world" }));
    assert_eq!(data.priority, 3);
    assert_eq!(data.run_at, now);
    assert_eq!(data.attempts, 4);
    assert_eq!(data.max_attempts, 5);
    assert_eq!(data.last_error.as_deref(), Some("last error"));
    assert_eq!(data.created_at, now);
    assert_eq!(data.updated_at, now);
    assert_eq!(data.key.as_deref(), Some("job-key"));
    assert_eq!(data.revision, 6);
    assert_eq!(data.locked_at, Some(now));
    assert_eq!(data.locked_by.as_deref(), Some("worker"));
    assert_eq!(data.flags, Some(serde_json::json!({ "flag": true })));
    assert_eq!(data.task_id, 8);
}
