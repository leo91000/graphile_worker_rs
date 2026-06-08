use chrono::Utc;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::LocalQueueMode;
use tracing::error;

use graphile_worker_queries::get_job::get_job;

use super::LocalQueue;

impl LocalQueue {
    pub async fn get_job(&self, flags_to_skip: &[String]) -> Option<Job> {
        let mode = *self.0.mode.read().await;
        if mode == LocalQueueMode::Released {
            return None;
        }

        if !flags_to_skip.is_empty() {
            return self.get_job_direct(flags_to_skip).await;
        }

        self.get_job_from_cache().await
    }

    async fn get_job_direct(&self, flags_to_skip: &[String]) -> Option<Job> {
        let task_details = self.0.task_details.read().await;
        let now = self.0.use_local_time.then(Utc::now);
        match get_job(
            &self.0.database,
            &task_details,
            &self.0.schema,
            &self.0.worker_id,
            flags_to_skip,
            now,
        )
        .await
        {
            Ok(job) => job,
            Err(e) => {
                error!(error = %e, "LocalQueue direct get_job failed");
                None
            }
        }
    }

    async fn get_job_from_cache(&self) -> Option<Job> {
        {
            let mode = *self.0.mode.read().await;
            if mode == LocalQueueMode::TtlExpired {
                self.set_mode(LocalQueueMode::Polling).await;
            }
            if mode == LocalQueueMode::Released {
                return None;
            }
        }

        let mut job_queue = self.0.job_queue.lock().await;
        if let Some(job) = job_queue.pop_front() {
            let remaining = job_queue.len();
            drop(job_queue);

            if remaining == 0 {
                self.set_mode(LocalQueueMode::Polling).await;
            }

            return Some(job);
        }

        None
    }
}
