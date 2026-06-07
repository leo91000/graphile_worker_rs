use std::time::Duration;

use futures::FutureExt;
use graphile_worker_lifecycle_hooks::{
    LocalQueueMode, LocalQueueRefetchDelayAbortContext, LocalQueueRefetchDelayExpiredContext,
    LocalQueueRefetchDelayStartContext,
};
use graphile_worker_runtime as runtime;
use rand::RngExt;
use tracing::trace;

use super::{LocalQueue, RefetchDelayConfig};

impl LocalQueue {
    pub(super) async fn start_refetch_delay(&self, config: &RefetchDelayConfig) {
        let max_abort = config.max_abort_threshold.unwrap_or(5 * self.0.config.size);
        let abort_threshold = if max_abort == usize::MAX {
            usize::MAX
        } else {
            let random: f64 = rand::rng().random();
            ((random * max_abort as f64) as usize).max(1)
        };

        *self.0.refetch_delay.abort_threshold.write().await = abort_threshold;
        self.set_refetch_delay_active(true);
        self.set_refetch_delay_fetch_on_complete(false);

        let duration = Duration::from_millis(
            ((0.5 + rand::rng().random::<f64>() * 0.5) * config.duration.as_millis() as f64) as u64,
        );

        trace!(
            ?duration,
            abort_threshold,
            "LocalQueue starting refetch delay"
        );

        self.0
            .hooks
            .emit(LocalQueueRefetchDelayStartContext {
                worker_id: self.0.worker_id.clone(),
                duration,
                threshold: config.threshold,
                abort_threshold,
            })
            .await;

        let queue_clone = self.clone();
        let refetch_delay_task = runtime::spawn(async move {
            let sleep = runtime::sleep(duration).fuse();
            let notified = queue_clone.0.refetch_delay.abort_notify.notified().fuse();
            futures::pin_mut!(sleep, notified);
            futures::select_biased! {
                _ = sleep => {
                    queue_clone.refetch_delay_complete(false).await;
                }
                _ = notified => {
                    queue_clone.refetch_delay_complete(true).await;
                }
            };
        });
        self.0.refetch_delay_task.replace_abort(refetch_delay_task);
    }

    async fn refetch_delay_complete(&self, aborted: bool) {
        self.set_refetch_delay_active(false);
        self.0.state_notify.notify_one();

        if aborted {
            let count = self.get_refetch_delay_counter();
            let abort_threshold = *self.0.refetch_delay.abort_threshold.read().await;

            self.set_refetch_delay_fetch_on_complete(true);
            trace!("LocalQueue refetch delay aborted");

            self.0
                .hooks
                .emit(LocalQueueRefetchDelayAbortContext {
                    worker_id: self.0.worker_id.clone(),
                    count,
                    abort_threshold,
                })
                .await;
        } else {
            trace!("LocalQueue refetch delay expired");

            self.0
                .hooks
                .emit(LocalQueueRefetchDelayExpiredContext {
                    worker_id: self.0.worker_id.clone(),
                })
                .await;
        }
    }

    async fn check_refetch_delay_abort(&self) {
        if !self.is_refetch_delay_active() {
            return;
        }

        let counter = self.get_refetch_delay_counter();
        let abort_threshold = *self.0.refetch_delay.abort_threshold.read().await;

        if counter >= abort_threshold {
            self.0.refetch_delay.abort_notify.notify_one();
        }
    }

    pub async fn pulse(&self, count: usize) {
        trace!(count, "LocalQueue received pulse");

        self.increment_refetch_delay_counter(count);
        self.check_refetch_delay_abort().await;

        let mode = *self.0.mode.read().await;

        match mode {
            LocalQueueMode::Polling => {
                if self.is_fetch_in_progress() {
                    self.set_fetch_again(true);
                    self.0.state_notify.notify_one();
                } else if !self.is_refetch_delay_active() {
                    self.0.state_notify.notify_one();
                }
            }
            LocalQueueMode::Waiting | LocalQueueMode::TtlExpired => {}
            LocalQueueMode::Released | LocalQueueMode::Starting => {}
        }
    }
}
