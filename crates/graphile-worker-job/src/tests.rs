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
