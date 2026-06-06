use std::sync::Arc;

use futures::FutureExt;
use graphile_worker_runtime::Notify;
use graphile_worker_shutdown_signal::ShutdownSignal;

/// Creates a shutdown signal that can be triggered manually via the returned notifier.
pub(super) fn manual_shutdown_signal_pair() -> (ShutdownSignal, Arc<Notify>) {
    let notify = Arc::new(Notify::new());
    let notify_for_signal = notify.clone();
    let signal = async move {
        notify_for_signal.notified().await;
    }
    .boxed()
    .shared();

    (signal, notify)
}

/// Resolves as soon as either of the provided shutdown signals completes.
pub(super) fn combine_shutdown_signals(
    left: ShutdownSignal,
    right: ShutdownSignal,
) -> ShutdownSignal {
    async move {
        let left = left.fuse();
        let right = right.fuse();
        futures::pin_mut!(left, right);
        futures::select_biased! {
            _ = left => (),
            _ = right => (),
        };
    }
    .boxed()
    .shared()
}
