use graphile_worker_database::{Database, Schema};
use tracing::error;

use graphile_worker_queries::fail_job::batch::{fail_jobs, FailedJob};
use graphile_worker_queries::fail_job::single::fail_job;

use super::FailureRequest;

pub(super) struct FailureBatchResult {
    persisted: Vec<bool>,
}

impl FailureBatchResult {
    pub(super) fn persisted(&self) -> &[bool] {
        &self.persisted
    }
}

pub(super) async fn persist_failure_batch(
    batch: &[FailureRequest],
    database: &Database,
    schema: &Schema,
    worker_id: &str,
) -> FailureBatchResult {
    let failed_jobs: Vec<FailedJob<'_>> = batch
        .iter()
        .map(|req| FailedJob {
            job: req.job.as_ref(),
            error: &req.error,
        })
        .collect();

    if let Err(error) = fail_jobs(database, &failed_jobs, schema, worker_id).await {
        error!(?error, batch_size = batch.len(), "Failed to fail jobs");

        let mut persisted = Vec::with_capacity(batch.len());
        for req in batch {
            persisted.push(fail_job_direct(req, database, schema, worker_id).await);
        }
        return FailureBatchResult { persisted };
    }

    FailureBatchResult {
        persisted: vec![true; batch.len()],
    }
}

pub(super) async fn fail_job_direct(
    req: &FailureRequest,
    database: &Database,
    schema: &Schema,
    worker_id: &str,
) -> bool {
    if let Err(error) = fail_job(database, &req.job, schema, worker_id, &req.error, None).await {
        error!(
            ?error,
            job_id = ?req.job.id(),
            "Failed to fail job directly"
        );
        return false;
    }

    true
}
