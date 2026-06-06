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
    let schema = setup_schema(&pg_pool, "completion_fallback").await;
    let utils = crate::worker_utils::WorkerUtils::new(pg_pool.clone(), schema.clone());
    let job = utils
        .add_raw_job(
            "completion_fallback_job",
            serde_json::json!({}),
            crate::JobSpec::default(),
        )
        .await
        .expect("Failed to add completion fallback job");
    let job_id = *job.id();
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
            has_queue: false,
            job: Arc::new(job),
            duration: Duration::ZERO,
        })
        .await;

    assert_eq!(hook_count.load(Ordering::SeqCst), 1);

    let remaining: (i64,) = safe_query_as(format!(
        "SELECT COUNT(*) FROM {schema}._private_jobs WHERE id = $1"
    ))
    .bind(job_id)
    .fetch_one(&pg_pool)
    .await
    .expect("Failed to count completed fallback job");
    assert_eq!(remaining.0, 0);

    drop_schema(&pg_pool, &schema).await;
}

#[tokio::test]
async fn failure_batcher_falls_back_after_shutdown() {
    let Some(pg_pool) = database_pool() else {
        return;
    };
    let schema = setup_schema(&pg_pool, "failure_fallback").await;
    let utils = crate::worker_utils::WorkerUtils::new(pg_pool.clone(), schema.clone());
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
