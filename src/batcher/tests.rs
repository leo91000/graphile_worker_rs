use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_lifecycle_hooks::{HookRegistry, JobComplete, JobFail};
use graphile_worker_shutdown_signal::ShutdownSignal;
use sqlx::postgres::{PgArguments, PgPoolOptions, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{FromRow, PgPool, Postgres};

use super::{CompletionBatcher, CompletionRequest, FailureBatcher, FailureRequest};

fn safe_query(sql: impl Into<String>) -> Query<'static, Postgres, PgArguments> {
    sqlx::query(sqlx::AssertSqlSafe(sql.into()))
}

fn safe_query_as<T>(sql: impl Into<String>) -> QueryAs<'static, Postgres, T, PgArguments>
where
    T: for<'row> FromRow<'row, PgRow>,
{
    sqlx::query_as(sqlx::AssertSqlSafe(sql.into()))
}

static SCHEMA_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn database_pool() -> Option<PgPool> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(&database_url)
        .ok()
}

async fn setup_schema(pg_pool: &PgPool, prefix: &str) -> String {
    let schema = format!(
        "graphile_worker_{prefix}_{}_{}",
        std::process::id(),
        SCHEMA_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    graphile_worker_migrations::migrate(pg_pool, &schema)
        .await
        .expect("Failed to migrate fallback test schema");
    schema
}

async fn drop_schema(pg_pool: &PgPool, schema: &str) {
    safe_query(format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
        .execute(pg_pool)
        .await
        .expect("Failed to drop fallback test schema");
}

fn completion_hooks(counter: Arc<AtomicUsize>) -> Arc<HookRegistry> {
    let mut hooks = HookRegistry::new();
    hooks.on(JobComplete, move |_ctx| {
        let counter = counter.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    Arc::new(hooks)
}

fn failure_hooks(counter: Arc<AtomicUsize>) -> Arc<HookRegistry> {
    let mut hooks = HookRegistry::new();
    hooks.on(JobFail, move |_ctx| {
        let counter = counter.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    Arc::new(hooks)
}

fn ready_shutdown_signal() -> ShutdownSignal {
    futures::future::ready(()).boxed().shared()
}

#[tokio::test]
async fn completion_batcher_falls_back_after_shutdown() {
    let Some(pg_pool) = database_pool() else {
        return;
    };
    for has_queue in [false, true] {
        let schema = setup_schema(&pg_pool, "completion_fallback").await;
        let utils = crate::worker_utils::client::WorkerUtils::new(pg_pool.clone(), schema.clone());
        let job = utils
            .add_raw_job(
                "completion_fallback_job",
                serde_json::json!({}),
                crate::JobSpec {
                    queue_name: has_queue.then(|| "completion_fallback_queue".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add completion fallback job");
        let job_id = *job.id();
        if has_queue {
            safe_query(format!(
                "UPDATE {schema}._private_job_queues SET locked_by='worker', locked_at=now()"
            ))
            .execute(&pg_pool)
            .await
            .unwrap();
        }
        let hook_count = Arc::new(AtomicUsize::new(0));

        let batcher = CompletionBatcher::new(
            Duration::from_secs(60),
            pg_pool.clone(),
            schema.clone(),
            "worker".to_string(),
            completion_hooks(hook_count.clone()),
            ready_shutdown_signal(),
        );
        batcher.await_shutdown().await;

        batcher
            .complete(CompletionRequest {
                job_id,
                has_queue,
                job: Arc::new(job),
                duration: Duration::ZERO,
            })
            .await
            .expect("direct fallback completion must persist successfully");

        assert_eq!(hook_count.load(Ordering::SeqCst), 1);

        let remaining: (i64,) = safe_query_as(format!(
            "SELECT COUNT(*) FROM {schema}._private_jobs WHERE id = $1"
        ))
        .bind(job_id)
        .fetch_one(&pg_pool)
        .await
        .expect("Failed to count completed fallback job");
        assert_eq!(remaining.0, 0);

        if has_queue {
            let queue: (Option<String>, bool) = safe_query_as(format!(
                "SELECT locked_by, locked_at IS NOT NULL FROM {schema}._private_job_queues"
            ))
            .fetch_one(&pg_pool)
            .await
            .unwrap();
            assert_eq!(
                queue,
                (None, false),
                "successful fallback releases the named queue"
            );
        }

        drop_schema(&pg_pool, &schema).await;
    }
    pg_pool.close().await;
}

#[tokio::test]
async fn completion_batcher_acknowledges_enqueue_before_persistence() {
    let Some(pg_pool) = database_pool() else {
        return;
    };
    let schema = setup_schema(&pg_pool, "completion_enqueue").await;
    let utils = crate::worker_utils::client::WorkerUtils::new(pg_pool.clone(), schema.clone());
    let job = utils
        .add_raw_job(
            "completion_enqueue_job",
            serde_json::json!({}),
            crate::JobSpec::default(),
        )
        .await
        .unwrap();
    let job_id = *job.id();
    let completed = Arc::new(AtomicUsize::new(0));
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_signal = async move {
        let _ = shutdown_rx.await;
    }
    .boxed()
    .shared();
    let batcher = CompletionBatcher::new(
        Duration::from_secs(60),
        pg_pool.clone(),
        schema.clone(),
        "worker".into(),
        completion_hooks(completed.clone()),
        shutdown_signal,
    );
    batcher
        .complete(CompletionRequest {
            job_id,
            has_queue: false,
            job: Arc::new(job),
            duration: Duration::ZERO,
        })
        .await
        .expect("open channel acknowledges enqueueing");
    assert_eq!(completed.load(Ordering::SeqCst), 0);
    let remaining: (i64,) = safe_query_as(format!(
        "SELECT count(*) FROM {schema}._private_jobs WHERE id=$1"
    ))
    .bind(job_id)
    .fetch_one(&pg_pool)
    .await
    .unwrap();
    assert_eq!(remaining.0, 1);
    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), batcher.await_shutdown())
        .await
        .expect("pending completions drain");
    assert_eq!(completed.load(Ordering::SeqCst), 1);
    let remaining: (i64,) = safe_query_as(format!(
        "SELECT count(*) FROM {schema}._private_jobs WHERE id=$1"
    ))
    .bind(job_id)
    .fetch_one(&pg_pool)
    .await
    .unwrap();
    assert_eq!(remaining.0, 0);
    drop_schema(&pg_pool, &schema).await;
    pg_pool.close().await;
}

#[tokio::test]
async fn failure_batcher_falls_back_after_shutdown() {
    let Some(pg_pool) = database_pool() else {
        return;
    };
    let schema = setup_schema(&pg_pool, "failure_fallback").await;
    let utils = crate::worker_utils::client::WorkerUtils::new(pg_pool.clone(), schema.clone());
    let job = utils
        .add_raw_job(
            "failure_fallback_job",
            serde_json::json!({}),
            crate::JobSpec::default(),
        )
        .await
        .expect("Failed to add failure fallback job");
    let job_id = *job.id();
    safe_query(format!(
        "UPDATE {schema}._private_jobs SET locked_by = $1, locked_at = now() WHERE id = $2"
    ))
    .bind("worker")
    .bind(job_id)
    .execute(&pg_pool)
    .await
    .expect("Failed to lock failure fallback job");
    let hook_count = Arc::new(AtomicUsize::new(0));

    let batcher = FailureBatcher::new(
        Duration::from_secs(60),
        pg_pool.clone(),
        schema.clone(),
        "worker".to_string(),
        failure_hooks(hook_count.clone()),
        ready_shutdown_signal(),
    );
    batcher.await_shutdown().await;

    batcher
        .fail(FailureRequest {
            job: Arc::new(job),
            error: "direct failure".to_string(),
            will_retry: true,
        })
        .await;

    assert_eq!(hook_count.load(Ordering::SeqCst), 1);

    let row: (Option<String>, Option<String>) = safe_query_as(format!(
        "SELECT last_error, locked_by FROM {schema}._private_jobs WHERE id = $1"
    ))
    .bind(job_id)
    .fetch_one(&pg_pool)
    .await
    .expect("Failed to fetch failed fallback job");
    assert_eq!(row.0.as_deref(), Some("direct failure"));
    assert!(row.1.is_none());

    drop_schema(&pg_pool, &schema).await;
}

#[derive(Clone, Debug, Default)]
struct CompletionGate {
    started: Arc<tokio::sync::Notify>,
    finish: Arc<tokio::sync::Notify>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ShutdownCompletionJob {}

impl crate::TaskHandler for ShutdownCompletionJob {
    const IDENTIFIER: &'static str = "shutdown_completion_job";

    async fn run(self, ctx: crate::WorkerContext) -> impl crate::IntoTaskHandlerResult {
        let gate = ctx.get_ext::<CompletionGate>().unwrap().clone();
        gate.started.notify_one();
        gate.finish.notified().await;
    }
}

#[tokio::test]
async fn worker_reports_direct_completion_failure_during_shutdown() {
    use crate::errors::GraphileWorkerError;
    use crate::runner::{ReleaseJobError, WorkerRuntimeError};

    let Some(pg_pool) = database_pool() else {
        return;
    };
    for has_queue in [false, true] {
        let schema = setup_schema(&pg_pool, "completion_error").await;
        let gate = CompletionGate::default();
        let completed = Arc::new(AtomicUsize::new(0));
        let worker = Arc::new(
            crate::Worker::options()
                .database(pg_pool.clone())
                .schema(schema.clone())
                .concurrency(1)
                .poll_interval(Duration::from_millis(10))
                .use_notification_delivery(false)
                .listen_os_shutdown_signals(false)
                .shutdown_grace_period(Duration::from_secs(5))
                .complete_job_batch_delay(Duration::from_secs(60))
                .define_job::<ShutdownCompletionJob>()
                .add_extension(gate.clone())
                .on(JobComplete, {
                    let completed = completed.clone();
                    move |_| {
                        let completed = completed.clone();
                        async move {
                            completed.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                })
                .init()
                .await
                .unwrap(),
        );
        let job = worker
            .create_utils()
            .add_job(
                ShutdownCompletionJob {},
                crate::JobSpec {
                    queue_name: has_queue.then(|| "shutdown_completion_queue".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let job_id = *job.id();
        safe_query(format!("CREATE FUNCTION {schema}.reject_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION USING ERRCODE='40001', MESSAGE='injected completion failure'; END $$"))
            .execute(&pg_pool).await.unwrap();
        safe_query(format!("CREATE TRIGGER reject_completion BEFORE DELETE ON {schema}._private_jobs FOR EACH ROW EXECUTE FUNCTION {schema}.reject_completion()"))
            .execute(&pg_pool).await.unwrap();

        let runner = worker.clone();
        let run = tokio::spawn(async move { runner.run().await });
        tokio::time::timeout(Duration::from_secs(5), gate.started.notified())
            .await
            .expect("handler started");
        worker.request_shutdown();
        // Wait for the shutdown listener to close the channel while the handler
        // is still running in its grace period, then finish the handler.
        tokio::time::timeout(
            Duration::from_secs(5),
            worker.completion_batcher.as_ref().unwrap().await_shutdown(),
        )
        .await
        .expect("batcher shut down");
        gate.finish.notify_one();
        let error = tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .expect("worker shutdown finished")
            .expect("worker task completed")
            .expect_err("Worker::run must report the failed fallback completion");
        let WorkerRuntimeError::ProcessJob(error) = error else {
            panic!("unexpected runtime error: {error:?}");
        };
        let error = std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<ReleaseJobError>()
            .expect("release error in runtime source chain");
        assert!(error.to_string().contains(&format!("job '{job_id}'")));
        let source = std::error::Error::source(error)
            .unwrap()
            .downcast_ref::<GraphileWorkerError>()
            .expect("original query error in source chain");
        let GraphileWorkerError::SqlError(source) = source else {
            panic!("expected database error");
        };
        assert_eq!(source.code(), Some("40001"));
        assert!(source.to_string().contains("injected completion failure"));
        assert_eq!(completed.load(Ordering::SeqCst), 0);
        let remaining: (i16, Option<String>, bool) = safe_query_as(format!(
            "SELECT attempts,locked_by,locked_at IS NOT NULL FROM {schema}._private_jobs WHERE id=$1"
        )).bind(job_id).fetch_one(&pg_pool).await.unwrap();
        assert_eq!(remaining, (1, Some(worker.worker_id().clone()), true));
        if has_queue {
            let queue: (Option<String>, bool) = safe_query_as(format!(
                "SELECT locked_by,locked_at IS NOT NULL FROM {schema}._private_job_queues"
            ))
            .fetch_one(&pg_pool)
            .await
            .unwrap();
            assert_eq!(queue, (Some(worker.worker_id().clone()), true));
        }
        drop(worker);
        drop_schema(&pg_pool, &schema).await;
    }
    pg_pool.close().await;
}
