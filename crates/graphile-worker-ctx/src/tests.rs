use graphile_worker_extensions::{Extensions, ReadOnlyExtensions};
use graphile_worker_job::Job;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::{SharedTaskDetails, WorkerContext};

fn create_test_job() -> Job {
    Job::builder()
        .id(1)
        .payload(serde_json::json!({"test": "data"}))
        .task_identifier("test_task".to_string())
        .build()
}

fn create_test_pool() -> PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://test:test@localhost/test")
        .expect("Failed to create lazy pool")
}

fn create_extensions() -> ReadOnlyExtensions {
    ReadOnlyExtensions::new(Extensions::default())
}

#[derive(Clone, Debug)]
struct TestExtension {
    value: &'static str,
}

#[tokio::test]
async fn test_worker_context_builder() {
    let job = create_test_job();
    let pool = create_test_pool();
    let extensions = create_extensions();
    let task_details = SharedTaskDetails::default();

    let ctx = WorkerContext::builder()
        .payload(serde_json::json!({"key": "value"}))
        .pg_pool(pool)
        .schema("graphile_worker".to_string())
        .job(job)
        .worker_id("worker-1".to_string())
        .extensions(extensions)
        .task_details(task_details)
        .use_local_time(true)
        .build();

    assert_eq!(ctx.payload(), &serde_json::json!({"key": "value"}));
    assert_eq!(ctx.schema().escaped(), "graphile_worker");
    assert_eq!(ctx.worker_id(), "worker-1");
    assert!(ctx.use_local_time());
}

#[tokio::test]
async fn test_worker_context_builder_use_local_time_default() {
    let job = create_test_job();
    let pool = create_test_pool();
    let extensions = create_extensions();
    let task_details = SharedTaskDetails::default();

    let ctx = WorkerContext::builder()
        .payload(serde_json::json!({}))
        .pg_pool(pool)
        .schema("schema".to_string())
        .job(job)
        .worker_id("worker".to_string())
        .extensions(extensions)
        .task_details(task_details)
        .build();

    assert!(!ctx.use_local_time());
}

#[tokio::test]
async fn test_worker_context_from_shared_job_uses_job_payload() {
    let job = std::sync::Arc::new(create_test_job());
    let pool = create_test_pool();
    let mut extensions = Extensions::default();
    extensions.insert(TestExtension { value: "present" });
    let extensions = ReadOnlyExtensions::new(extensions);
    let task_details = SharedTaskDetails::default();

    let ctx = WorkerContext::from_shared_job(
        job.clone(),
        pool,
        "graphile_worker".to_string(),
        "worker-1".to_string(),
        extensions,
        task_details,
        true,
    );

    assert_eq!(ctx.payload(), job.payload());
    assert_eq!(ctx.job().id(), job.id());
    assert_eq!(ctx.schema().escaped(), "graphile_worker");
    assert_eq!(ctx.worker_id(), "worker-1");
    assert!(ctx.use_local_time());
    assert_eq!(
        ctx.extensions().get::<TestExtension>().unwrap().value,
        "present"
    );
    assert_eq!(ctx.get_ext::<TestExtension>().unwrap().value, "present");
}

#[tokio::test]
async fn test_worker_context_builder_uses_job_payload_when_payload_missing() {
    let job = create_test_job();
    let expected_payload = job.payload().clone();
    let pool = create_test_pool();
    let extensions = create_extensions();
    let task_details = SharedTaskDetails::default();

    let ctx = WorkerContext::builder()
        .pg_pool(pool)
        .schema("schema".to_string())
        .job(job)
        .worker_id("worker".to_string())
        .extensions(extensions)
        .task_details(task_details)
        .build();

    assert_eq!(ctx.payload(), &expected_payload);
}
