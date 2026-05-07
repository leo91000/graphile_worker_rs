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
use std::time::Instant;
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
            hooks,
            shutdown_signal,
        ));

        Self {
            tx,
            task: runtime::Mutex::new(Some(task)),
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

        let deadline = Instant::now() + delay;
        loop {
            let wait_item = runtime::timeout_at(deadline, rx.recv()).fuse();
            let shutdown = (&mut shutdown_signal).fuse();
            futures::pin_mut!(wait_item, shutdown);

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
                result = wait_item => result,
            };

            match result {
                Ok(Ok(item)) => batch.push(item),
                Ok(Err(_)) => {
                    flush_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks).await;
                    return;
                }
                Err(_) => break,
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

    if !with_queue_ids.is_empty() {
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

        if let Err(e) = sqlx::query(&sql)
            .bind(&with_queue_ids)
            .bind(worker_id)
            .execute(pg_pool)
            .await
        {
            error!(error = ?e, "Failed to complete jobs with queue");
        }
    }

    if !without_queue_ids.is_empty() {
        let sql = formatdoc!(
            r#"
                DELETE FROM {escaped_schema}._private_jobs
                WHERE id = ANY($1::bigint[])
            "#
        );

        if let Err(e) = sqlx::query(&sql)
            .bind(&without_queue_ids)
            .execute(pg_pool)
            .await
        {
            error!(error = ?e, "Failed to complete jobs without queue");
        }
    }

    if !hooks.is_empty() {
        let worker_id = worker_id.to_string();
        for req in batch {
            hooks
                .emit(JobCompleteContext {
                    job: req.job.clone(),
                    worker_id: worker_id.clone(),
                    duration: req.duration,
                })
                .await;
        }
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
    tx: runtime::Sender<FailureRequest>,
    task: runtime::Mutex<Option<runtime::JoinHandle<()>>>,
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
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);

        let task = runtime::spawn(failure_batcher_task(
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
            task: runtime::Mutex::new(Some(task)),
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

        let deadline = Instant::now() + delay;
        loop {
            let wait_item = runtime::timeout_at(deadline, rx.recv()).fuse();
            let shutdown = (&mut shutdown_signal).fuse();
            futures::pin_mut!(wait_item, shutdown);

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
                result = wait_item => result,
            };

            match result {
                Ok(Ok(item)) => batch.push(item),
                Ok(Err(_)) => {
                    flush_failure_batch(&batch, &pg_pool, &escaped_schema, &worker_id, &hooks)
                        .await;
                    return;
                }
                Err(_) => break,
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

    if let Err(e) = fail_jobs(pg_pool, &failed_jobs, escaped_schema, worker_id).await {
        error!(error = ?e, batch_size = batch.len(), "Failed to fail jobs");

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
    }

    if !hooks.is_empty() {
        let worker_id = worker_id.to_string();
        for req in batch {
            if req.will_retry {
                hooks
                    .emit(JobFailContext {
                        job: req.job.clone(),
                        worker_id: worker_id.clone(),
                        error: req.error.clone(),
                        will_retry: true,
                    })
                    .await;
            } else {
                hooks
                    .emit(JobPermanentlyFailContext {
                        job: req.job.clone(),
                        worker_id: worker_id.clone(),
                        error: req.error.clone(),
                    })
                    .await;
            }
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
