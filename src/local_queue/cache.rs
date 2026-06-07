use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::LocalQueueMode;
use graphile_worker_runtime as runtime;
use tracing::trace;

use super::LocalQueue;

impl LocalQueue {
    pub(super) async fn received_jobs(&self, jobs: Vec<Job>, fetched_max: bool) {
        let job_count = jobs.len();
        {
            let mut job_queue = self.0.job_queue.lock().await;
            job_queue.extend(jobs);
        }

        self.set_mode(LocalQueueMode::Waiting).await;
        self.start_ttl_timer().await;

        trace!(job_count, "Jobs added to cache, signaling stream");

        let _ = self.0.job_signal_sender.try_send(());

        if fetched_max {
            self.set_fetch_again(true);
        }
    }

    async fn start_ttl_timer(&self) {
        let ttl = self.0.config.ttl;

        let queue_clone = self.clone();
        let ttl_timer_task = runtime::spawn(async move {
            runtime::sleep(ttl).await;

            let mode = *queue_clone.0.mode.read().await;
            if mode == LocalQueueMode::Waiting {
                queue_clone.set_mode_ttl_expired().await;
            }
        });

        self.0.ttl_timer_task.replace_abort(ttl_timer_task);
    }
}
