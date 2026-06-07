use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{Database, Schema};
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::{trace, warn};

use super::shared::{run_batcher_task, BatchProcessor};
use super::BATCHER_CHANNEL_CAPACITY;
use crate::background_tasks::TaskSlot;
use crate::Job;

mod hooks;
mod persistence;

use hooks::emit_failure_hook;
use persistence::{fail_job_direct, persist_failure_batch};

pub struct FailureRequest {
    pub job: Arc<Job>,
    pub error: String,
    pub will_retry: bool,
}

pub struct FailureBatcher {
    tx: runtime::Sender<FailureRequest>,
    task: TaskSlot,
    database: Database,
    schema: Schema,
    worker_id: String,
    hooks: Arc<HookRegistry>,
}

impl FailureBatcher {
    pub fn new(
        delay: Duration,
        database: impl Into<Database>,
        schema: impl Into<Schema>,
        worker_id: String,
        hooks: Arc<HookRegistry>,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        let database = database.into();
        let schema = schema.into();
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);
        let processor = FailureProcessor {
            database: database.clone(),
            schema: schema.clone(),
            worker_id: worker_id.clone(),
            hooks: hooks.clone(),
        };

        let task = runtime::spawn(run_batcher_task(rx, delay, processor, shutdown_signal));

        Self {
            tx,
            task: TaskSlot::new("failure_batcher", task),
            database,
            schema,
            worker_id,
            hooks,
        }
    }

    pub async fn fail(&self, req: FailureRequest) {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, failing job directly");
            let req = e.0;
            if fail_job_direct(&req, &self.database, &self.schema, &self.worker_id).await {
                emit_failure_hook(&req, &self.worker_id, &self.hooks).await;
            }
        }
    }

    pub async fn await_shutdown(&self) {
        self.task.stop().await;
    }
}

struct FailureProcessor {
    database: Database,
    schema: Schema,
    worker_id: String,
    hooks: Arc<HookRegistry>,
}

impl BatchProcessor<FailureRequest> for FailureProcessor {
    fn flush<'a>(
        &'a self,
        batch: &'a [FailureRequest],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            flush_failure_batch(
                batch,
                &self.database,
                &self.schema,
                &self.worker_id,
                &self.hooks,
            )
            .await;
        })
    }
}

async fn flush_failure_batch(
    batch: &[FailureRequest],
    database: &Database,
    schema: &Schema,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    if batch.is_empty() {
        return;
    }

    trace!(batch_size = batch.len(), "Flushing failure batch");

    let batch_result = persist_failure_batch(batch, database, schema, worker_id).await;
    if hooks.is_empty() {
        return;
    }

    for (req, persisted) in batch.iter().zip(batch_result.persisted()) {
        if *persisted {
            emit_failure_hook(req, worker_id, hooks).await;
        }
    }
}
