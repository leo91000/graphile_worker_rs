use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_database::{
    BoxFuture, Database, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
    NotificationStream,
};
use graphile_worker_extensions::{Extensions, ReadOnlyExtensions};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_shutdown_signal::ShutdownSignal;

use crate::streams::StreamSource;

use super::errors::{Redacted, RunJobError};
use super::release::release_job;
use super::task_execution::panic_payload_to_string;
use super::WorkerRunner;

#[derive(Debug)]
struct FailingDriver;

impl DbExecutor for FailingDriver {
    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async { Err(DbError::new("forced failure")) })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

impl DatabaseDriver for FailingDriver {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn begin<'a>(&'a self) -> BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async { Err(DbError::new("transactions are unavailable")) })
    }

    fn listen<'a>(
        &'a self,
        _channel: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async { Ok(None) })
    }
}

fn pending_shutdown_signal() -> ShutdownSignal {
    futures::future::pending::<()>().boxed().shared()
}

#[tokio::test]
async fn failing_driver_contract_is_exercised() {
    let driver = FailingDriver;

    assert!(driver.as_any().is::<FailingDriver>());
    assert!(driver
        .execute("", DbParams::new())
        .await
        .expect_err("execute should fail")
        .to_string()
        .contains("forced failure"));
    assert!(driver
        .fetch_all("", DbParams::new())
        .await
        .expect("fetch_all should return an empty result")
        .is_empty());
    assert!(driver.begin().await.is_err());
    assert!(driver
        .listen("")
        .await
        .expect("listen should succeed without a stream")
        .is_none());
}

#[test]
fn panic_payload_to_string_handles_static_str() {
    assert_eq!(
        panic_payload_to_string(Box::new("static panic")),
        "static panic"
    );
}

#[test]
fn run_job_error_debug_redacts_replacement_payload() {
    let payload = serde_json::json!({ "secret": "token" });
    let error = RunJobError::TaskErrorWithReplacement {
        message: "failed".to_string(),
        replacement_payload: Redacted::new(payload.clone()),
    };

    let debug = format!("{error:?}");

    assert!(debug.contains("[redacted]"));
    assert!(!debug.contains("token"));
    assert_eq!(format!("{}", Redacted::new(payload.clone())), "[redacted]");
    assert_eq!(error.persisted_error(), "TaskError(\"failed\")");
    assert_eq!(error.replacement_payload(), Some(&payload));
}

#[test]
fn panic_payload_to_string_handles_unknown_payload() {
    assert_eq!(panic_payload_to_string(Box::new(1usize)), "task panicked");
}

#[test]
fn panic_payload_to_string_handles_string_payload() {
    assert_eq!(
        panic_payload_to_string(Box::new(String::from("dynamic panic"))),
        "dynamic panic"
    );
}

#[test]
fn stream_source_is_copy_for_worker_fanout() {
    fn assert_copy<T: Copy>() {}

    assert_copy::<StreamSource>();
}

#[tokio::test]
async fn release_job_returns_error_when_replacement_payload_cannot_be_persisted() {
    let database = Database::new(FailingDriver);
    let hooks = Arc::new(HookRegistry::default());
    let shutdown_signal = pending_shutdown_signal();
    let failure_batcher = Arc::new(crate::batcher::FailureBatcher::new(
        Duration::from_secs(60),
        database.clone(),
        "graphile_worker".to_string(),
        "worker".to_string(),
        hooks.clone(),
        shutdown_signal.clone(),
    ));
    let worker = WorkerRunner {
        worker_id: "worker".to_string(),
        jobs: HashMap::new(),
        database,
        schema: graphile_worker_database::Schema::new("graphile_worker"),
        task_details: Default::default(),
        forbidden_flags: Vec::new(),
        use_local_time: false,
        shutdown_signal,
        extensions: ReadOnlyExtensions::from(Extensions::new()),
        hooks,
        completion_batcher: None,
        failure_batcher: Some(failure_batcher),
        shutdown_config: crate::WorkerShutdownConfig::default(),
    };
    let job = Arc::new(
        Job::builder()
            .id(42)
            .attempts(1)
            .max_attempts(1)
            .locked_by("worker".to_string())
            .task_identifier("batch")
            .payload(serde_json::json!({ "items": [1] }))
            .build(),
    );

    let error = release_job(
        Err(RunJobError::TaskErrorWithReplacement {
            message: "failed".to_string(),
            replacement_payload: Redacted::new(serde_json::json!({ "items": [2] })),
        }),
        job,
        &worker,
        Duration::from_millis(5),
    )
    .await
    .expect_err("release should surface the failed persistence query");

    assert_eq!(error.job_id, 42);
    assert!(error.source.to_string().contains("forced failure"));
}
