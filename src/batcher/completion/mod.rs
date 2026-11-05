use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{Database, Schema};
use graphile_worker_lifecycle_hooks::{HookRegistry, JobCompleteContext};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::{trace, warn};

use super::shared::{run_batcher_task, BatchProcessor};
use super::BATCHER_CHANNEL_CAPACITY;
use crate::background_tasks::TaskSlot;
use crate::errors::Result;
use crate::Job;

mod persistence;
use persistence::{complete_batch, complete_job_direct};

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
    schema: Schema,
    worker_id: String,
    hooks: Arc<HookRegistry>,
    queue_wakeup: Arc<crate::queue_wakeup::QueueWakeup>,
}

impl CompletionBatcher {
    /// Starts a batcher sharing the worker's post-persistence queue wakeups.
    pub fn new(
        delay: Duration,
        database: impl Into<Database>,
        schema: impl Into<Schema>,
        worker_id: String,
        hooks: Arc<HookRegistry>,
        shutdown_signal: ShutdownSignal,
        queue_wakeup: Arc<crate::queue_wakeup::QueueWakeup>,
    ) -> Self {
        let database = database.into();
        let schema = schema.into();
        let (tx, rx) = runtime::channel(BATCHER_CHANNEL_CAPACITY);
        let processor = CompletionProcessor {
            database: database.clone(),
            schema: schema.clone(),
            worker_id: worker_id.clone(),
            hooks: hooks.clone(),
            queue_wakeup: queue_wakeup.clone(),
        };

        let task = runtime::spawn(run_batcher_task(rx, delay, processor, shutdown_signal));

        Self {
            tx,
            task: TaskSlot::new("completion_batcher", task),
            database,
            schema,
            worker_id,
            hooks,
            queue_wakeup,
        }
    }

    /// Accepts a request for batching, or persists it directly after closure.
    ///
    /// An open channel acknowledges enqueueing. Direct fallback errors are
    /// returned to the caller; the success hook requires persisted completion.
    pub async fn complete(&self, req: CompletionRequest) -> Result<()> {
        if let Err(e) = self.tx.send(req).await {
            warn!("Batcher closed, completing job directly");
            let req = e.0;
            complete_job_direct(&req, &self.database, &self.schema, &self.worker_id).await?;
            if req.has_queue {
                self.queue_wakeup.wake(1);
            }
            emit_completion_hook(&req, &self.worker_id, &self.hooks).await;
        }
        Ok(())
    }

    pub async fn await_shutdown(&self) {
        self.task.stop().await;
    }
}

struct CompletionProcessor {
    database: Database,
    schema: Schema,
    worker_id: String,
    hooks: Arc<HookRegistry>,
    queue_wakeup: Arc<crate::queue_wakeup::QueueWakeup>,
}

impl BatchProcessor<CompletionRequest> for CompletionProcessor {
    fn flush<'a>(
        &'a self,
        batch: &'a [CompletionRequest],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            flush_batch(
                batch,
                &self.database,
                &self.schema,
                &self.worker_id,
                &self.hooks,
                &self.queue_wakeup,
            )
            .await;
        })
    }
}

/// Persists both completion groups and wakes fetchers only for successful
/// named outcomes, before running application observers.
async fn flush_batch(
    batch: &[CompletionRequest],
    database: &Database,
    schema: &Schema,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
    queue_wakeup: &crate::queue_wakeup::QueueWakeup,
) {
    if batch.is_empty() {
        return;
    }

    trace!(batch_size = batch.len(), "Flushing completion batch");

    let batch_result = complete_batch(batch, database, schema, worker_id).await;

    queue_wakeup.wake(
        batch
            .iter()
            .filter(|req| req.has_queue && batch_result.persisted(req))
            .count(),
    );

    if !hooks.is_empty() {
        for req in batch {
            if batch_result.persisted(req) {
                emit_completion_hook(req, worker_id, hooks).await;
            }
        }
    }
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
