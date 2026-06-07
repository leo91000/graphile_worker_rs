use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

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

pub(super) trait BatchProcessor<T>: Send + Sync + 'static {
    fn flush<'a>(&'a self, batch: &'a [T]) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

pub(super) async fn run_batcher_task<T, P>(
    rx: runtime::Receiver<T>,
    delay: Duration,
    processor: P,
    mut shutdown_signal: ShutdownSignal,
) where
    T: Send + 'static,
    P: BatchProcessor<T>,
{
    let mut batch = Vec::new();

    loop {
        let first = recv_or_shutdown(&rx, &mut shutdown_signal).await;

        if first.shutdown {
            drain_and_flush(&rx, &mut batch, &processor).await;
            return;
        }

        let Some(first) = first.item else {
            processor.flush(&batch).await;
            return;
        };

        batch.push(first);

        let timeout = runtime::sleep(delay).fuse();
        futures::pin_mut!(timeout);

        loop {
            let recv = rx.recv().fuse();
            let shutdown = (&mut shutdown_signal).fuse();
            futures::pin_mut!(recv, shutdown);

            let result = futures::select_biased! {
                _ = shutdown => {
                    drain_and_flush(&rx, &mut batch, &processor).await;
                    return;
                }
                _ = timeout => break,
                result = recv => result,
            };

            match result {
                Ok(item) => batch.push(item),
                Err(_) => {
                    processor.flush(&batch).await;
                    return;
                }
            }
        }

        processor.flush(&batch).await;
        batch.clear();
    }
}

async fn drain_and_flush<T, P>(rx: &runtime::Receiver<T>, batch: &mut Vec<T>, processor: &P)
where
    T: Send + 'static,
    P: BatchProcessor<T>,
{
    while let Ok(item) = rx.try_recv() {
        batch.push(item);
    }
    processor.flush(batch).await;
}
