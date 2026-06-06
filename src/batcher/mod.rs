use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_database::{Database, DbExecutor, DbValue};
use graphile_worker_lifecycle_hooks::{
    HookRegistry, JobCompleteContext, JobFailContext, JobPermanentlyFailContext,
};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use indoc::formatdoc;
use tracing::{error, trace, warn};

use crate::background_tasks::TaskSlot;
use crate::sql::fail_job::{fail_job, fail_jobs, FailedJob};
use crate::Job;

mod shared;
use shared::recv_or_shutdown;

const BATCHER_CHANNEL_CAPACITY: usize = 4096;

pub struct CompletionRequest {
    pub job_id: i64,
    pub has_queue: bool,
    pub job: Arc<Job>,
    pub duration: Duration,
}

pub struct CompletionBatcher {
    tx: runtime::Sender<CompletionRequest>,
    task: TaskSlot,
    database: Database,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
}

impl CompletionBatcher {
    pub fn new(
        delay: Duration,
        database: impl Into<Database>,
        escaped_schema: String,
        worker_id: String,
        hooks: Arc<HookRegistry>,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        let database = database.into();
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);

        let task = runtime::spawn(completion_batcher_task(
            rx,
            delay,
            database.clone(),
            escaped_schema.clone(),
            worker_id.clone(),
            hooks.clone(),
            shutdown_signal,
        ));

        Self {
            tx,
            task: TaskSlot::new("completion_batcher", task),
            database,
            escaped_schema,
            worker_id,
            hooks,
        }
    }

    pub async fn complete(&self, req: CompletionRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, completing job directly");
            let req = e.0;
            if complete_job_direct(&req, &self.database, &self.escaped_schema, &self.worker_id)
                .await
            {
                emit_completion_hook(&req, &self.worker_id, &self.hooks).await;
            }
        }
    }

    pub async fn await_shutdown(&self) {
        self.task.stop().await;
    }
}

async fn completion_batcher_task(
    rx: runtime::Receiver<CompletionRequest>,
    delay: Duration,
    database: Database,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
    mut shutdown_signal: ShutdownSignal,
) {
    let mut batch: Vec<CompletionRequest> = Vec::new();

    loop {
        let first = recv_or_shutdown(&rx, &mut shutdown_signal).await;

        if first.shutdown {
            drain_and_flush(
                &rx,
                &mut batch,
                &database,
                &escaped_schema,
                &worker_id,
                &hooks,
            )
            .await;
            return;
        }

        let Some(first) = first.item else {
            flush_batch(&batch, &database, &escaped_schema, &worker_id, &hooks).await;
            return;
        };

        batch.push(first);

        let timeout = runtime::sleep(delay).fuse();
        futures::pin_mut!(timeout);

        loop {
            let recv = rx.recv().fuse();
            let shutdown = (&mut shutdown_signal).fuse();
            futures::pin_mut!(recv, shutdown);

            let result = futures::select_biased! {
                _ = shutdown => {
                    drain_and_flush(
                        &rx,
                        &mut batch,
                        &database,
                        &escaped_schema,
                        &worker_id,
                        &hooks,
                    ).await;
                    return;
                }
                _ = timeout => break,
                result = recv => result,
            };

            match result {
                Ok(item) => batch.push(item),
                Err(_) => {
                    flush_batch(&batch, &database, &escaped_schema, &worker_id, &hooks).await;
                    return;
                }
            }
        }

        flush_batch(&batch, &database, &escaped_schema, &worker_id, &hooks).await;
        batch.clear();
    }
}

async fn drain_and_flush(
    rx: &runtime::Receiver<CompletionRequest>,
    batch: &mut Vec<CompletionRequest>,
    database: &Database,
    escaped_schema: &str,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    while let Ok(item) = rx.try_recv() {
        batch.push(item);
    }
    flush_batch(batch, database, escaped_schema, worker_id, hooks).await;
}

async fn flush_batch(
    batch: &[CompletionRequest],
    database: &Database,
    escaped_schema: &str,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    if batch.is_empty() {
        return;
    }

    trace!(batch_size = batch.len(), "Flushing completion batch");

    let mut with_queue_ids = Vec::new();
    let mut without_queue_ids = Vec::with_capacity(batch.len());

    for req in batch {
        if req.has_queue {
            with_queue_ids.push(req.job_id);
        } else {
            without_queue_ids.push(req.job_id);
        }
    }

    let with_queue_succeeded = if !with_queue_ids.is_empty() {
        let sql = formatdoc!(
            r#"
                WITH j AS (
                    DELETE FROM {escaped_schema}._private_jobs
                    WHERE id = ANY($1::bigint[])
                    RETURNING *
                )
                UPDATE {escaped_schema}._private_job_queues AS job_queues
                SET locked_by = NULL, locked_at = NULL
                FROM j
                WHERE job_queues.id = j.job_queue_id AND job_queues.locked_by = $2::text
            "#
        );

        match database
            .execute(
                &sql,
                vec![
                    DbValue::I64Array(with_queue_ids),
                    DbValue::Text(worker_id.to_string()),
                ]
                .into(),
            )
            .await
        {
            Ok(_) => true,
            Err(e) => {
                error!(error = ?e, "Failed to complete jobs with queue");
                false
            }
        }
    } else {
        false
    };

    let without_queue_succeeded = if !without_queue_ids.is_empty() {
        let sql = formatdoc!(
            r#"
                DELETE FROM {escaped_schema}._private_jobs
                WHERE id = ANY($1::bigint[])
            "#
        );

        match database
            .execute(&sql, vec![DbValue::I64Array(without_queue_ids)].into())
            .await
        {
            Ok(_) => true,
            Err(e) => {
                error!(error = ?e, "Failed to complete jobs without queue");
                false
            }
        }
    } else {
        false
    };

    if !hooks.is_empty() {
        for req in batch {
            let persisted = (req.has_queue && with_queue_succeeded)
                || (!req.has_queue && without_queue_succeeded);
            if persisted {
                emit_completion_hook(req, worker_id, hooks).await;
            }
        }
    }
}

async fn complete_job_direct(
    req: &CompletionRequest,
    database: &Database,
    escaped_schema: &str,
    worker_id: &str,
) -> bool {
    if req.has_queue {
        let sql = formatdoc!(
            r#"
                WITH j AS (
                    DELETE FROM {escaped_schema}._private_jobs
                    WHERE id = $1
                    RETURNING *
                )
                UPDATE {escaped_schema}._private_job_queues AS job_queues
                SET locked_by = NULL, locked_at = NULL
                FROM j
                WHERE job_queues.id = j.job_queue_id AND job_queues.locked_by = $2::text
            "#
        );

        if let Err(e) = database
            .execute(
                &sql,
                vec![
                    DbValue::I64(req.job_id),
                    DbValue::Text(worker_id.to_string()),
                ]
                .into(),
            )
            .await
        {
            error!(error = ?e, job_id = req.job_id, "Failed to complete job directly (with queue)");
            return false;
        }
    } else {
        let sql = formatdoc!(
            r#"
                DELETE FROM {escaped_schema}._private_jobs
                WHERE id = $1
            "#
        );

        if let Err(e) = database
            .execute(&sql, vec![DbValue::I64(req.job_id)].into())
            .await
        {
            error!(error = ?e, job_id = req.job_id, "Failed to complete job directly");
            return false;
        }
    }

    true
}

async fn emit_completion_hook(req: &CompletionRequest, worker_id: &str, hooks: &Arc<HookRegistry>) {
    if hooks.is_empty() {
        return;
    }

    hooks
        .emit(JobCompleteContext {
            job: req.job.clone(),
            worker_id: worker_id.to_string(),
            duration: req.duration,
        })
        .await;
}

pub struct FailureRequest {
    pub job: Arc<Job>,
    pub error: String,
    pub will_retry: bool,
}

pub struct FailureBatcher {
    tx: runtime::Sender<FailureRequest>,
    task: TaskSlot,
    database: Database,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
}

impl FailureBatcher {
    pub fn new(
        delay: Duration,
        database: impl Into<Database>,
        escaped_schema: String,
        worker_id: String,
        hooks: Arc<HookRegistry>,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        let database = database.into();
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);

        let task = runtime::spawn(failure_batcher_task(
            rx,
            delay,
            database.clone(),
            escaped_schema.clone(),
            worker_id.clone(),
            hooks.clone(),
            shutdown_signal,
        ));

        Self {
            tx,
            task: TaskSlot::new("failure_batcher", task),
            database,
            escaped_schema,
            worker_id,
            hooks,
        }
    }

    pub async fn fail(&self, req: FailureRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, failing job directly");
            let req = e.0;
            if fail_job_direct(&req, &self.database, &self.escaped_schema, &self.worker_id).await {
                emit_failure_hook(&req, &self.worker_id, &self.hooks).await;
            }
        }
    }

    pub async fn await_shutdown(&self) {
        self.task.stop().await;
    }
}

async fn failure_batcher_task(
    rx: runtime::Receiver<FailureRequest>,
    delay: Duration,
    database: Database,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
    mut shutdown_signal: ShutdownSignal,
) {
    let mut batch: Vec<FailureRequest> = Vec::new();

    loop {
        let first = recv_or_shutdown(&rx, &mut shutdown_signal).await;

        if first.shutdown {
            drain_and_flush_failures(
                &rx,
                &mut batch,
                &database,
                &escaped_schema,
                &worker_id,
                &hooks,
            )
            .await;
            return;
        }

        let Some(first) = first.item else {
            flush_failure_batch(&batch, &database, &escaped_schema, &worker_id, &hooks).await;
            return;
        };

        batch.push(first);

        let timeout = runtime::sleep(delay).fuse();
        futures::pin_mut!(timeout);

        loop {
            let recv = rx.recv().fuse();
            let shutdown = (&mut shutdown_signal).fuse();
            futures::pin_mut!(recv, shutdown);

            let result = futures::select_biased! {
                _ = shutdown => {
                    drain_and_flush_failures(
                        &rx,
                        &mut batch,
                        &database,
                        &escaped_schema,
                        &worker_id,
                        &hooks,
                    ).await;
                    return;
                }
                _ = timeout => break,
                result = recv => result,
            };

            match result {
                Ok(item) => batch.push(item),
                Err(_) => {
                    flush_failure_batch(&batch, &database, &escaped_schema, &worker_id, &hooks)
                        .await;
                    return;
                }
            }
        }

        flush_failure_batch(&batch, &database, &escaped_schema, &worker_id, &hooks).await;
        batch.clear();
    }
}

async fn drain_and_flush_failures(
    rx: &runtime::Receiver<FailureRequest>,
    batch: &mut Vec<FailureRequest>,
    database: &Database,
    escaped_schema: &str,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    while let Ok(item) = rx.try_recv() {
        batch.push(item);
    }
    flush_failure_batch(batch, database, escaped_schema, worker_id, hooks).await;
}

async fn flush_failure_batch(
    batch: &[FailureRequest],
    database: &Database,
    escaped_schema: &str,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    if batch.is_empty() {
        return;
    }

    trace!(batch_size = batch.len(), "Flushing failure batch");

    let failed_jobs: Vec<FailedJob<'_>> = batch
        .iter()
        .map(|req| FailedJob {
            job: req.job.as_ref(),
            error: &req.error,
        })
        .collect();

    match fail_jobs(database, &failed_jobs, escaped_schema, worker_id).await {
        Ok(()) => {
            if !hooks.is_empty() {
                for req in batch {
                    emit_failure_hook(req, worker_id, hooks).await;
                }
            }
        }
        Err(e) => {
            error!(error = ?e, batch_size = batch.len(), "Failed to fail jobs");

            for req in batch {
                if fail_job_direct(req, database, escaped_schema, worker_id).await {
                    emit_failure_hook(req, worker_id, hooks).await;
                }
            }
        }
    }
}

async fn emit_failure_hook(req: &FailureRequest, worker_id: &str, hooks: &Arc<HookRegistry>) {
    if hooks.is_empty() {
        return;
    }

    if req.will_retry {
        hooks
            .emit(JobFailContext {
                job: req.job.clone(),
                worker_id: worker_id.to_string(),
                error: req.error.clone(),
                will_retry: true,
            })
            .await;
    } else {
        hooks
            .emit(JobPermanentlyFailContext {
                job: req.job.clone(),
                worker_id: worker_id.to_string(),
                error: req.error.clone(),
            })
            .await;
    }
}

async fn fail_job_direct(
    req: &FailureRequest,
    database: &Database,
    escaped_schema: &str,
    worker_id: &str,
) -> bool {
    if let Err(e) = fail_job(
        database,
        &req.job,
        escaped_schema,
        worker_id,
        &req.error,
        None,
    )
    .await
    {
        error!(error = ?e, job_id = ?req.job.id(), "Failed to fail job directly");
        return false;
    }

    true
}
#[cfg(all(test, feature = "driver-sqlx"))]
mod tests {
    use super::*;
    use futures::FutureExt;
    use graphile_worker_lifecycle_hooks::{JobComplete, JobFail};
    use sqlx::{postgres::PgPoolOptions, PgPool};
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP SCHEMA IF EXISTS {schema} CASCADE"
        )))
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
        let pg_pool = database_pool().expect("DATABASE_URL is required for fallback tests");
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

        let remaining: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {schema}._private_jobs WHERE id = $1"
        )))
        .bind(job_id)
        .fetch_one(&pg_pool)
        .await
        .expect("Failed to count completed fallback job");
        assert_eq!(remaining.0, 0);

        drop_schema(&pg_pool, &schema).await;
    }

    #[tokio::test]
    async fn failure_batcher_falls_back_after_shutdown() {
        let pg_pool = database_pool().expect("DATABASE_URL is required for fallback tests");
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
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE {schema}._private_jobs SET locked_by = $1, locked_at = now() WHERE id = $2"
        )))
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

        let row: (Option<String>, Option<String>) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT last_error, locked_by FROM {schema}._private_jobs WHERE id = $1"
        )))
        .bind(job_id)
        .fetch_one(&pg_pool)
        .await
        .expect("Failed to fetch failed fallback job");
        assert_eq!(row.0.as_deref(), Some("direct failure"));
        assert!(row.1.is_none());

        drop_schema(&pg_pool, &schema).await;
    }
}
