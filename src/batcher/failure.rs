use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_database::Database;
use graphile_worker_lifecycle_hooks::{HookRegistry, JobFailContext, JobPermanentlyFailContext};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::{error, trace, warn};

use super::shared::recv_or_shutdown;
use super::BATCHER_CHANNEL_CAPACITY;
use crate::background_tasks::TaskSlot;
use crate::sql::fail_job::{fail_job, fail_jobs, FailedJob};
use crate::Job;

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
