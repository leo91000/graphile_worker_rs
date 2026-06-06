use futures::FutureExt;
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;

pub(super) struct RecvOrShutdown<T> {
    pub(super) item: Option<T>,
    pub(super) shutdown: bool,
}

pub(super) async fn recv_or_shutdown<T>(
    rx: &runtime::Receiver<T>,
    shutdown_signal: &mut ShutdownSignal,
) -> RecvOrShutdown<T> {
    let recv = rx.recv().fuse();
    let shutdown = shutdown_signal.fuse();
    futures::pin_mut!(recv, shutdown);

    futures::select_biased! {
        _ = shutdown => RecvOrShutdown { item: None, shutdown: true },
        item = recv => RecvOrShutdown { item: item.ok(), shutdown: false },
    }
}
