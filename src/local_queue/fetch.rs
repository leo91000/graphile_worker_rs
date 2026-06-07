use chrono::Utc;
use futures::FutureExt;
use graphile_worker_lifecycle_hooks::{LocalQueueGetJobsCompleteContext, LocalQueueMode};
use graphile_worker_runtime as runtime;
use tracing::{debug, error};

use crate::sql::batch_get_jobs::batch_get_jobs;

use super::LocalQueue;

impl LocalQueue {
    pub(super) async fn schedule_fetch(&self) {
        loop {
            let mode = *self.0.mode.read().await;

            if mode == LocalQueueMode::Released {
                break;
            }

            let can_fetch = mode == LocalQueueMode::Polling
                && !self.is_fetch_in_progress()
                && !self.is_refetch_delay_active();

            if !can_fetch {
                self.0.state_notify.notified().await;
                continue;
            }

            self.fetch().await;

            let mode = *self.0.mode.read().await;
            if mode == LocalQueueMode::Polling && self.0.continuous {
                let should_fetch_again = self.take_fetch_again();
                let refetch_delay_wants_fetch = self.take_refetch_delay_fetch_on_complete();

                if !should_fetch_again && !refetch_delay_wants_fetch {
                    let sleep = runtime::sleep(self.0.poll_interval).fuse();
                    let notified = self.0.state_notify.notified().fuse();
                    futures::pin_mut!(sleep, notified);
                    futures::select_biased! {
                        _ = sleep => {}
                        _ = notified => {}
                    };
                }
            } else if mode == LocalQueueMode::Polling && !self.0.continuous {
                self.set_mode(LocalQueueMode::Released).await;
                break;
            }
        }
    }

    async fn fetch(&self) {
        if !self.try_start_fetch() {
            return;
        }

        if self.is_refetch_delay_active() {
            self.set_refetch_delay_fetch_on_complete(true);
            self.end_fetch();
            self.0.state_notify.notify_one();
            return;
        }

        self.set_fetch_again(false);
        self.reset_refetch_delay_counter();

        let task_details = self.0.task_details.read().await;
        let now = self.0.use_local_time.then(Utc::now);
        let result = batch_get_jobs(
            &self.0.database,
            &task_details,
            &self.0.schema,
            &self.0.worker_id,
            &[],
            self.0.config.size.try_into().unwrap_or(i32::MAX),
            now,
        )
        .await;
        drop(task_details);

        self.end_fetch();
        self.0.state_notify.notify_one();

        match result {
            Ok(jobs) => {
                let job_count = jobs.len();
                debug!(job_count, "LocalQueue fetched jobs from database");

                self.0
                    .hooks
                    .emit(LocalQueueGetJobsCompleteContext {
                        worker_id: self.0.worker_id.clone(),
                        jobs_count: job_count,
                    })
                    .await;

                let fetched_max = job_count >= self.0.config.size;

                if let Some(ref refetch_delay_config) = self.0.config.refetch_delay {
                    let threshold_surpassed =
                        fetched_max || job_count > refetch_delay_config.threshold;

                    if !threshold_surpassed {
                        self.start_refetch_delay(refetch_delay_config).await;
                    }
                }

                if !jobs.is_empty() {
                    self.received_jobs(jobs, fetched_max).await;
                } else if !self.0.continuous {
                    self.set_mode(LocalQueueMode::Released).await;
                }
            }
            Err(e) => {
                error!(error = %e, "LocalQueue failed to fetch jobs");
            }
        }
    }
}
