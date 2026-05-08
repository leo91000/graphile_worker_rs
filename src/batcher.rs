use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_lifecycle_hooks::{
    HookRegistry, JobCompleteContext, JobFailContext, JobPermanentlyFailContext,
};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use indoc::formatdoc;
use sqlx::PgPool;
use tracing::{error, trace, warn};

use crate::sql::fail_job::{fail_job, fail_jobs, FailedJob};
use crate::Job;

const BATCHER_CHANNEL_CAPACITY: usize = 4096;

pub struct CompletionRequest {
    pub job_id: i64,
    pub has_queue: bool,
    pub job: Arc<Job>,
    pub duration: Duration,
}

pub struct CompletionBatcher {
    tx: runtime::Sender<CompletionRequest>,
    task: runtime::Mutex<Option<runtime::JoinHandle<()>>>,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
}

impl CompletionBatcher {
    pub fn new(
        delay: Duration,
        pg_pool: PgPool,
        escaped_schema: String,
        worker_id: String,
        hooks: Arc<HookRegistry>,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);

        let task = runtime::spawn(completion_batcher_task(
            rx,
            delay,
            pg_pool.clone(),
            escaped_schema.clone(),
            worker_id.clone(),
            hooks.clone(),
            shutdown_signal,
        ));

        Self {
            tx,
            task: runtime::Mutex::new(Some(task)),
            pg_pool,
            escaped_schema,
            worker_id,
            hooks,
        }
    }

    pub async fn complete(&self, req: CompletionRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, completing job directly");
            let req = e.0;
            if complete_job_direct(&req, &self.pg_pool, &self.escaped_schema, &self.worker_id).await
            {
                emit_completion_hook(&req, &self.worker_id, &self.hooks).await;
            }
        }
    }

    pub async fn await_shutdown(&self) {
        if let Some(handle) = self.task.lock().await.take() {
            if let Err(e) = handle.await {
                error!(error = ?e, "Completion batcher task panicked");
            }
        }
    }
}

async fn completion_batcher_task(
    rx: runtime::Receiver<CompletionRequest>,
    delay: Duration,
    pg_pool: PgPool,
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
                &pg_pool,
                &escaped_schema,
                &worker_id,
                &hooks,
            )
            .await;
            return;
        }

        let Some(first) = first.item else {
            flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
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
                        &pg_pool,
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
                    flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
                    return;
                }
            }
        }

        flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
        batch.clear();
    }
}

struct RecvOrShutdown<T> {
    item: Option<T>,
    shutdown: bool,
}

async fn recv_or_shutdown<T>(
    rx: &runtime::Receiver<T>,
    shutdown_signal: &mut ShutdownSignal,
) -> RecvOrShutdown<T> {
    let recv = rx.recv().fuse();
    let shutdown = shutdown_signal.fuse();
    futures::pin_mut!(recv, shutdown);

    futures::select_biased! {
        _ = shutdown => RecvOrShutdown { item: None, shutdown: true },
        item = recv => RecvOrShutdown { item: item.ok(), shutdown: false },
    }
}

async fn drain_and_flush(
    rx: &runtime::Receiver<CompletionRequest>,
    batch: &mut Vec<CompletionRequest>,
    pg_pool: &PgPool,
    escaped_schema: &str,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    while let Ok(item) = rx.try_recv() {
        batch.push(item);
    }
    flush_batch(batch, pg_pool, escaped_schema, worker_id, hooks).await;
}

async fn flush_batch(
    batch: &[CompletionRequest],
    pg_pool: &PgPool,
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

        match sqlx::query(&sql)
            .bind(&with_queue_ids)
            .bind(worker_id)
            .execute(pg_pool)
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

        match sqlx::query(&sql)
            .bind(&without_queue_ids)
            .execute(pg_pool)
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
    pg_pool: &PgPool,
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

        if let Err(e) = sqlx::query(&sql)
            .bind(req.job_id)
            .bind(worker_id)
            .execute(pg_pool)
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

        if let Err(e) = sqlx::query(&sql).bind(req.job_id).execute(pg_pool).await {
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
    task: runtime::Mutex<Option<runtime::JoinHandle<()>>>,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
}

impl FailureBatcher {
    pub fn new(
        delay: Duration,
        pg_pool: PgPool,
        escaped_schema: String,
        worker_id: String,
        hooks: Arc<HookRegistry>,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);

        let task = runtime::spawn(failure_batcher_task(
            rx,
            delay,
            pg_pool.clone(),
            escaped_schema.clone(),
            worker_id.clone(),
            hooks.clone(),
            shutdown_signal,
        ));

        Self {
            tx,
            task: runtime::Mutex::new(Some(task)),
            pg_pool,
            escaped_schema,
            worker_id,
            hooks,
        }
    }

    pub async fn fail(&self, req: FailureRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, failing job directly");
            let req = e.0;
            if fail_job_direct(&req, &self.pg_pool, &self.escaped_schema, &self.worker_id).await {
                emit_failure_hook(&req, &self.worker_id, &self.hooks).await;
            }
        }
    }

    pub async fn await_shutdown(&self) {
        if let Some(handle) = self.task.lock().await.take() {
            if let Err(e) = handle.await {
                error!(error = ?e, "Failure batcher task panicked");
            }
        }
    }
}

async fn failure_batcher_task(
    rx: runtime::Receiver<FailureRequest>,
    delay: Duration,
    pg_pool: PgPool,
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
                &pg_pool,
                &escaped_schema,
                &worker_id,
                &hooks,
            )
            .await;
            return;
        }

        let Some(first) = first.item else {
            flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
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
                        &pg_pool,
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
                    flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks)
                        .await;
                    return;
                }
            }
        }

        flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
        batch.clear();
    }
}

async fn drain_and_flush_failures(
    rx: &runtime::Receiver<FailureRequest>,
    batch: &mut Vec<FailureRequest>,
    pg_pool: &PgPool,
    escaped_schema: &str,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    while let Ok(item) = rx.try_recv() {
        batch.push(item);
    }
    flush_failure_batch(batch, pg_pool, escaped_schema, worker_id, hooks).await;
}

async fn flush_failure_batch(
    batch: &[FailureRequest],
    pg_pool: &PgPool,
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

    match fail_jobs(pg_pool, &failed_jobs, escaped_schema, worker_id).await {
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
                if fail_job_direct(req, pg_pool, escaped_schema, worker_id).await {
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
    pg_pool: &PgPool,
    escaped_schema: &str,
    worker_id: &str,
) -> bool {
    if let Err(e) = fail_job(
        pg_pool,
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
#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use sqlx::postgres::PgPoolOptions;

    fn database_pool() -> Option<PgPool> {
        let database_url = std::env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&database_url)
            .ok()
    }

    fn ready_shutdown_signal() -> ShutdownSignal {
        futures::future::ready(()).boxed().shared()
    }

    fn job(id: i64, job_queue_id: Option<i32>) -> Arc<Job> {
        let mut builder = Job::builder().id(id);
        if let Some(job_queue_id) = job_queue_id {
            builder = builder.job_queue_id(job_queue_id);
        }
        Arc::new(builder.task_identifier("test_job").build())
    }

    #[tokio::test]
    async fn completion_batcher_falls_back_after_shutdown() {
        let Some(pg_pool) = database_pool() else {
            return;
        };

        let batcher = CompletionBatcher::new(
            Duration::from_secs(60),
            pg_pool,
            "missing_schema".to_string(),
            "worker".to_string(),
            Arc::new(HookRegistry::default()),
            ready_shutdown_signal(),
        );
        batcher.await_shutdown().await;

        batcher
            .complete(CompletionRequest {
                job_id: 1,
                has_queue: true,
                job: job(1, Some(1)),
                duration: Duration::ZERO,
            })
            .await;
        batcher
            .complete(CompletionRequest {
                job_id: 2,
                has_queue: false,
                job: job(2, None),
                duration: Duration::ZERO,
            })
            .await;
    }

    #[tokio::test]
    async fn failure_batcher_falls_back_after_shutdown() {
        let Some(pg_pool) = database_pool() else {
            return;
        };

        let batcher = FailureBatcher::new(
            Duration::from_secs(60),
            pg_pool,
            "missing_schema".to_string(),
            "worker".to_string(),
            Arc::new(HookRegistry::default()),
            ready_shutdown_signal(),
        );
        batcher.await_shutdown().await;

        batcher
            .fail(FailureRequest {
                job: job(3, None),
                error: "direct failure".to_string(),
                will_retry: true,
            })
            .await;
    }
}
