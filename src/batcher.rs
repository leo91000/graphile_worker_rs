use std::sync::Arc;
use std::time::Duration;

use graphile_worker_lifecycle_hooks::{
    HookRegistry, JobCompleteContext, JobFailContext, JobPermanentlyFailContext,
};
use graphile_worker_shutdown_signal::ShutdownSignal;
use indoc::formatdoc;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{timeout_at, Instant};
use tracing::{error, trace, warn};

use crate::sql::fail_job::fail_job;
use crate::Job;

pub struct CompletionRequest {
    pub job_id: i64,
    pub has_queue: bool,
    pub job: Arc<Job>,
    pub duration: Duration,
}

pub struct CompletionBatcher {
    tx: mpsc::Sender<CompletionRequest>,
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
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
        let (tx, rx) = mpsc::channel::<CompletionRequest>(1024);

        let task = tokio::spawn(completion_batcher_task(
            rx,
            delay,
            pg_pool.clone(),
            escaped_schema.clone(),
            worker_id.clone(),
            hooks,
            shutdown_signal,
        ));

        Self {
            tx,
            task: tokio::sync::Mutex::new(Some(task)),
            pg_pool,
            escaped_schema,
            worker_id,
        }
    }

    pub async fn complete(&self, req: CompletionRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, completing job directly");
            let req = e.0;
            complete_job_direct(&req, &self.pg_pool, &self.escaped_schema, &self.worker_id).await;
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
    mut rx: mpsc::Receiver<CompletionRequest>,
    delay: Duration,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
    mut shutdown_signal: ShutdownSignal,
) {
    let mut batch: Vec<CompletionRequest> = Vec::new();

    loop {
        let first = tokio::select! {
            biased;
            _ = &mut shutdown_signal => {
                drain_and_flush(
                    &mut rx,
                    &mut batch,
                    &pg_pool,
                    &escaped_schema,
                    &worker_id,
                    &hooks,
                ).await;
                return;
            }
            item = rx.recv() => item,
        };

        let Some(first) = first else {
            flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
            return;
        };

        batch.push(first);

        let deadline = Instant::now() + delay;
        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_signal => {
                    drain_and_flush(
                        &mut rx,
                        &mut batch,
                        &pg_pool,
                        &escaped_schema,
                        &worker_id,
                        &hooks,
                    ).await;
                    return;
                }
                result = timeout_at(deadline, rx.recv()) => {
                    match result {
                        Ok(Some(item)) => batch.push(item),
                        Ok(None) => {
                            flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
                            return;
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
        batch.clear();
    }
}

async fn drain_and_flush(
    rx: &mut mpsc::Receiver<CompletionRequest>,
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

    let (with_queue, without_queue): (Vec<_>, Vec<_>) = batch.iter().partition(|r| r.has_queue);

    if !with_queue.is_empty() {
        let ids: Vec<i64> = with_queue.iter().map(|r| r.job_id).collect();
        let sql = formatdoc!(
            r#"
                WITH j AS (
                    DELETE FROM {escaped_schema}._private_jobs
                    USING unnest($1::bigint[]) n(n) WHERE id = n
                    RETURNING *
                )
                UPDATE {escaped_schema}._private_job_queues AS job_queues
                SET locked_by = NULL, locked_at = NULL
                FROM j
                WHERE job_queues.id = j.job_queue_id AND job_queues.locked_by = $2::text
            "#
        );

        if let Err(e) = sqlx::query(&sql)
            .bind(&ids)
            .bind(worker_id)
            .execute(pg_pool)
            .await
        {
            error!(error = ?e, "Failed to complete jobs with queue");
        }
    }

    if !without_queue.is_empty() {
        let ids: Vec<i64> = without_queue.iter().map(|r| r.job_id).collect();
        let sql = formatdoc!(
            r#"
                DELETE FROM {escaped_schema}._private_jobs
                USING unnest($1::bigint[]) n(n) WHERE id = n
            "#
        );

        if let Err(e) = sqlx::query(&sql).bind(&ids).execute(pg_pool).await {
            error!(error = ?e, "Failed to complete jobs without queue");
        }
    }

    for req in batch {
        hooks
            .emit(JobCompleteContext {
                job: req.job.clone(),
                worker_id: worker_id.to_string(),
                duration: req.duration,
            })
            .await;
    }
}

async fn complete_job_direct(
    req: &CompletionRequest,
    pg_pool: &PgPool,
    escaped_schema: &str,
    worker_id: &str,
) {
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
        }
    }
}

pub struct FailureRequest {
    pub job: Arc<Job>,
    pub error: String,
    pub will_retry: bool,
}

pub struct FailureBatcher {
    tx: mpsc::Sender<FailureRequest>,
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
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
        let (tx, rx) = mpsc::channel::<FailureRequest>(1024);

        let task = tokio::spawn(failure_batcher_task(
            rx,
            delay,
            pg_pool.clone(),
            escaped_schema.clone(),
            worker_id.clone(),
            hooks,
            shutdown_signal,
        ));

        Self {
            tx,
            task: tokio::sync::Mutex::new(Some(task)),
            pg_pool,
            escaped_schema,
            worker_id,
        }
    }

    pub async fn fail(&self, req: FailureRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, failing job directly");
            let req = e.0;
            fail_job_direct(&req, &self.pg_pool, &self.escaped_schema, &self.worker_id).await;
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
    mut rx: mpsc::Receiver<FailureRequest>,
    delay: Duration,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
    hooks: Arc<HookRegistry>,
    mut shutdown_signal: ShutdownSignal,
) {
    let mut batch: Vec<FailureRequest> = Vec::new();

    loop {
        let first = tokio::select! {
            biased;
            _ = &mut shutdown_signal => {
                drain_and_flush_failures(
                    &mut rx,
                    &mut batch,
                    &pg_pool,
                    &escaped_schema,
                    &worker_id,
                    &hooks,
                ).await;
                return;
            }
            item = rx.recv() => item,
        };

        let Some(first) = first else {
            flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
            return;
        };

        batch.push(first);

        let deadline = Instant::now() + delay;
        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_signal => {
                    drain_and_flush_failures(
                        &mut rx,
                        &mut batch,
                        &pg_pool,
                        &escaped_schema,
                        &worker_id,
                        &hooks,
                    ).await;
                    return;
                }
                result = timeout_at(deadline, rx.recv()) => {
                    match result {
                        Ok(Some(item)) => batch.push(item),
                        Ok(None) => {
                            flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
                            return;
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
        batch.clear();
    }
}

async fn drain_and_flush_failures(
    rx: &mut mpsc::Receiver<FailureRequest>,
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

    for req in batch {
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
            error!(error = ?e, job_id = ?req.job.id(), "Failed to fail job");
        }
    }

    for req in batch {
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
}

async fn fail_job_direct(
    req: &FailureRequest,
    pg_pool: &PgPool,
    escaped_schema: &str,
    worker_id: &str,
) {
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
    }
}
