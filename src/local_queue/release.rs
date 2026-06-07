use graphile_worker_database::{Database, Schema};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{LocalQueueMode, LocalQueueReturnJobsContext};
use graphile_worker_runtime as runtime;
use tracing::{debug, error, warn};

use crate::sql::return_jobs::batch::return_jobs;

use super::config::{calculate_retry_delay, RETURN_JOBS_RETRY_OPTIONS};
use super::{LocalQueue, LocalQueueError};

impl LocalQueue {
    async fn return_jobs_with_retry(
        database: &Database,
        jobs: &[Job],
        schema: &Schema,
        worker_id: &str,
    ) -> Result<(), LocalQueueError> {
        let mut attempt = 0u32;
        loop {
            match return_jobs(database, jobs, schema, worker_id).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    if attempt >= RETURN_JOBS_RETRY_OPTIONS.max_attempts {
                        return Err(LocalQueueError::ReturnJobsError(format!(
                            "Failed after {} attempts: {}",
                            attempt, e
                        )));
                    }
                    let delay = calculate_retry_delay(attempt - 1, &RETURN_JOBS_RETRY_OPTIONS);
                    warn!(
                        attempt,
                        max_attempts = RETURN_JOBS_RETRY_OPTIONS.max_attempts,
                        ?delay,
                        error = %e,
                        "Failed to return jobs, retrying"
                    );
                    runtime::sleep(delay).await;
                }
            }
        }
    }

    pub(super) async fn set_mode_ttl_expired(&self) {
        let mut mode = self.0.mode.write().await;
        if *mode != LocalQueueMode::Waiting {
            return;
        }
        *mode = LocalQueueMode::TtlExpired;
        drop(mode);

        debug!("LocalQueue TTL expired, returning jobs to database");

        let jobs: Vec<Job> = self.0.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            if let Err(e) = Self::return_jobs_with_retry(
                &self.0.database,
                &jobs,
                &self.0.schema,
                &self.0.worker_id,
            )
            .await
            {
                error!(error = %e, "Failed to return jobs after TTL expiry (exhausted retries)");
            } else {
                self.0
                    .hooks
                    .emit(LocalQueueReturnJobsContext {
                        worker_id: self.0.worker_id.clone(),
                        jobs_count,
                    })
                    .await;
            }
        }
    }

    pub async fn release(&self) -> Result<(), LocalQueueError> {
        let mut mode = self.0.mode.write().await;
        if *mode == LocalQueueMode::Released {
            return Ok(());
        }
        *mode = LocalQueueMode::Released;
        drop(mode);

        self.set_refetch_delay_active(false);
        self.0.refetch_delay.abort_notify.notify_waiters();
        self.0.state_notify.notify_waiters();

        self.0.ttl_timer_task.abort();
        self.0.refetch_delay_task.abort();

        debug!("LocalQueue releasing, returning jobs to database");

        let jobs: Vec<Job> = self.0.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            Self::return_jobs_with_retry(
                &self.0.database,
                &jobs,
                &self.0.schema,
                &self.0.worker_id,
            )
            .await?;

            self.0
                .hooks
                .emit(LocalQueueReturnJobsContext {
                    worker_id: self.0.worker_id.clone(),
                    jobs_count,
                })
                .await;
        }

        self.0.run_complete_notify.notified().await;

        Ok(())
    }
}
