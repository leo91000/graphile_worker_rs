use std::sync::atomic::{AtomicUsize, Ordering};

use graphile_worker_runtime as runtime;

/// Coalesces queue releases into one pending wakeup while retaining their count
/// for LocalQueue's refetch-delay abort threshold. It never owns job claims.
#[derive(Default)]
pub(crate) struct QueueWakeup {
    notify: runtime::Notify,
    count: AtomicUsize,
}

impl QueueWakeup {
    /// Records only successfully persisted named outcomes; zero is a no-op.
    pub(crate) fn wake(&self, count: usize) {
        if count == 0 {
            return;
        }
        self.count
            .try_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                Some(pending.saturating_add(count))
            })
            .expect("the counter update always succeeds");
        self.notify.notify_one();
    }

    /// Waits for and consumes all accumulated release hints, including those
    /// emitted before the fetch loop starts listening.
    pub(crate) async fn next(&self) -> usize {
        loop {
            self.notify.notified().await;
            let count = self.count.swap(0, Ordering::AcqRel);
            if count > 0 {
                return count;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;

    use super::*;

    /// Wakeup storage is bounded, counts survive coalescing, and empty/stale
    /// permits cannot cause repeated polls when there is no new release.
    #[tokio::test]
    async fn coalesces_release_counts_without_losing_early_hints() {
        let wakeup = QueueWakeup::default();
        wakeup.wake(0);
        assert!(wakeup.next().now_or_never().is_none());
        wakeup.wake(2);
        wakeup.wake(3);
        assert_eq!(wakeup.next().await, 5);
        assert!(wakeup.next().now_or_never().is_none());
        wakeup.wake(usize::MAX);
        wakeup.wake(1);
        assert_eq!(wakeup.next().await, usize::MAX);
        wakeup.notify.notify_one();
        assert!(wakeup.next().now_or_never().is_none());
        wakeup.wake(1);
        assert_eq!(wakeup.next().await, 1);
    }
}
