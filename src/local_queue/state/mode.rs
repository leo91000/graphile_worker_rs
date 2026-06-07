use graphile_worker_lifecycle_hooks::{LocalQueueMode, LocalQueueSetModeContext};
use tracing::trace;

use super::super::LocalQueue;

impl LocalQueue {
    pub(in crate::local_queue) async fn set_mode(&self, new_mode: LocalQueueMode) {
        let mut mode = self.0.mode.write().await;
        let old_mode = *mode;
        if old_mode == new_mode {
            return;
        }
        trace!(?old_mode, ?new_mode, "LocalQueue mode transition");
        *mode = new_mode;
        drop(mode);

        self.0
            .hooks
            .emit(LocalQueueSetModeContext {
                worker_id: self.0.worker_id.clone(),
                old_mode,
                new_mode,
            })
            .await;

        self.0.state_notify.notify_waiters();
    }
}
