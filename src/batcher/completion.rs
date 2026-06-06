use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_database::{Database, DbExecutor, DbValue};
use graphile_worker_lifecycle_hooks::{HookRegistry, JobCompleteContext};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use indoc::formatdoc;
use tracing::{error, trace, warn};

use super::shared::recv_or_shutdown;
use super::BATCHER_CHANNEL_CAPACITY;
use crate::background_tasks::TaskSlot;
use crate::Job;

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
