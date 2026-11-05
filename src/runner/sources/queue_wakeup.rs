use std::sync::Arc;

use futures::{stream, FutureExt, Stream};

use graphile_worker_shutdown_signal::ShutdownSignal;

use crate::local_queue::LocalQueue;
use crate::queue_wakeup::QueueWakeup;
use crate::streams::job_signal::JobSignalSource;

/// Adds bounded, process-local queue-release hints without adding public signal
/// variants. Direct slots poll again; local queues receive pulses independently
/// of busy slots, retaining their existing refetch delay and terminal state.
pub(in crate::runner) fn with_queue_wakeups<S>(
    signals: S,
    notify: Arc<QueueWakeup>,
    shutdown: ShutdownSignal,
    local_queues: Arc<Vec<LocalQueue>>,
) -> impl Stream<Item = JobSignalSource>
where
    S: Stream<Item = JobSignalSource>,
{
    let wakeups = stream::unfold(
        (notify, shutdown, local_queues),
        |(notify, mut shutdown, queues)| async move {
            let count = futures::select_biased! {
                _ = (&mut shutdown).fuse() => return None,
                count = notify.next().fuse() => count,
            };
            for queue in queues.iter() {
                queue.pulse(count).await;
            }
            let source = if queues.is_empty() {
                JobSignalSource::Polling
            } else {
                JobSignalSource::LocalQueue
            };
            Some((source, (notify, shutdown, queues)))
        },
    );
    stream::select(signals, wakeups)
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    /// Pending hints are consumed without polling timers; shutdown wins even
    /// with another queued hint, and no closed-channel assertion is involved.
    #[tokio::test]
    async fn release_hints_poll_direct_slots_and_shutdown_discards_pending_hints() {
        let wakeup = Arc::new(QueueWakeup::default());
        let (shutdown_tx, shutdown_rx) = futures::channel::oneshot::channel::<()>();
        let shutdown = async move {
            let _ = shutdown_rx.await;
        }
        .boxed()
        .shared();
        let signals = with_queue_wakeups(
            stream::empty(),
            wakeup.clone(),
            shutdown,
            Arc::new(Vec::new()),
        );
        futures::pin_mut!(signals);
        wakeup.wake(2);
        assert_eq!(signals.next().await, Some(JobSignalSource::Polling));
        wakeup.wake(1);
        shutdown_tx.send(()).unwrap();
        assert_eq!(signals.next().await, None);
        wakeup.wake(1);
    }
}
