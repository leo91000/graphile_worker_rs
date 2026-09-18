use std::sync::atomic::Ordering;

use graphile_worker_database::{Database, Schema};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{LocalQueueMode, LocalQueueReturnJobsContext};
use graphile_worker_runtime as runtime;
use tracing::{debug, error, warn};

use graphile_worker_queries::return_jobs::batch::return_jobs;

use super::config::{calculate_retry_delay, RETURN_JOBS_RETRY_OPTIONS};
use super::{LocalQueue, LocalQueueError};

impl LocalQueue {
    /// Retries transient return failures using the bounded, jittered retry policy.
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

    /// Returns cached claims when a waiting queue reaches its TTL.
    pub(super) async fn set_mode_ttl_expired(&self) {
        let mut mode = self.0.mode.write().await;
        if *mode != LocalQueueMode::Waiting {
            return;
        }
        *mode = LocalQueueMode::TtlExpired;
        drop(mode);

        debug!("LocalQueue TTL expired, returning jobs to database");

        if let Err(error) = self.return_cached_jobs().await {
            error!(error = %error, "Failed to return jobs after TTL expiry (exhausted retries)");
        }
    }

    /// Retains claims until their return succeeds, including if this future is cancelled.
    async fn return_cached_jobs(&self) -> Result<(), LocalQueueError> {
        // Claims whose return may have committed must not reach a handler.
        // Keep them separately from the consumable cache until an idempotent
        // return succeeds. Cancellation retains this queue-owned backlog.
        let mut pending = self.0.pending_returns.lock().await;
        pending.extend(self.0.job_queue.lock().await.drain(..));
        if pending.is_empty() {
            return Ok(());
        }
        Self::return_jobs_with_retry(
            &self.0.database,
            &pending,
            &self.0.schema,
            &self.0.worker_id,
        )
        .await?;
        let jobs_count = pending.len();
        pending.clear();
        drop(pending);
        self.0
            .hooks
            .emit(LocalQueueReturnJobsContext {
                worker_id: self.0.worker_id.clone(),
                jobs_count,
            })
            .await;
        Ok(())
    }

    /// Stops fetching and returns cached claims before completing shutdown.
    ///
    /// Concurrent callers wait for the same cleanup. An in-flight fetch finishes
    /// before the cache is drained, and Released is terminal. If returning jobs
    /// exhausts its retries, the error is returned and the claims remain pending for
    /// a later release attempt.
    pub async fn release(&self) -> Result<(), LocalQueueError> {
        // Concurrent callers must all observe completed cleanup, not just the
        // transition to Released. A cancelled caller leaves this false for retry.
        let mut release_complete = self.0.release_complete.lock().await;
        if *release_complete {
            return Ok(());
        }
        let mut mode = self.0.mode.write().await;
        *mode = LocalQueueMode::Released;
        drop(mode);

        self.set_refetch_delay_active(false);
        self.0.refetch_delay.abort_notify.notify_waiters();
        // Store a permit if the fetch loop has checked its mode but has not yet
        // registered its waiter.
        self.0.state_notify.notify_one();

        // Stop a TTL return before awaiting fetch completion. Cancellation keeps
        // its backlog available to the final return below.
        self.0.ttl_timer_task.abort();

        // An in-flight fetch may still append jobs to the cache. Wait for it
        // before draining; Released is terminal so it cannot restart the loop.
        loop {
            let complete = self.0.run_complete_notify.notified();
            if self.0.run_complete.load(Ordering::Acquire) {
                break;
            }
            complete.await;
        }

        self.0.ttl_timer_task.abort();
        self.0.refetch_delay_task.abort();

        debug!("LocalQueue releasing, returning jobs to database");

        self.return_cached_jobs().await?;

        *release_complete = true;

        Ok(())
    }
}
