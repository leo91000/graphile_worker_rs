use graphile_worker_database::{Database, Schema};

use super::CompletionRequest;

mod queued;
mod unqueued;

use queued::{complete_jobs_with_queues, complete_one_job_with_queue};
use unqueued::{complete_jobs_without_queues, complete_one_job_without_queue};

pub(super) struct CompletionBatchResult {
    with_queue_succeeded: bool,
    without_queue_succeeded: bool,
}

impl CompletionBatchResult {
    pub(super) fn persisted(&self, req: &CompletionRequest) -> bool {
        (req.has_queue && self.with_queue_succeeded)
            || (!req.has_queue && self.without_queue_succeeded)
    }
}

pub(super) async fn complete_batch(
    batch: &[CompletionRequest],
    database: &Database,
    schema: &Schema,
    worker_id: &str,
) -> CompletionBatchResult {
    let mut with_queue_ids = Vec::new();
    let mut without_queue_ids = Vec::with_capacity(batch.len());

    for req in batch {
        if req.has_queue {
            with_queue_ids.push(req.job_id);
        } else {
            without_queue_ids.push(req.job_id);
        }
    }

    CompletionBatchResult {
        with_queue_succeeded: complete_jobs_with_queues(
            database,
            schema,
            worker_id,
            with_queue_ids,
        )
        .await,
        without_queue_succeeded: complete_jobs_without_queues(database, schema, without_queue_ids)
            .await,
    }
}

pub(super) async fn complete_job_direct(
    req: &CompletionRequest,
    database: &Database,
    schema: &Schema,
    worker_id: &str,
) -> bool {
    if req.has_queue {
        return complete_one_job_with_queue(req, database, schema, worker_id).await;
    }

    complete_one_job_without_queue(req, database, schema).await
}
