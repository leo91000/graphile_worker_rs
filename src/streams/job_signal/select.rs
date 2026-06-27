use futures::{FutureExt, StreamExt};
use graphile_worker_database::{DbError, Notification, NotificationStream};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::warn;

use super::state::{JobSignalStreamData, NextSignal};
use super::JobSignalSource;

pub(super) async fn next_signal(data: &mut JobSignalStreamData) -> NextSignal {
    if let Some(ref mut rx) = data.local_queue_rx {
        return next_signal_with_local_queue(
            &mut data.interval,
            data.pg_listener.as_mut(),
            &mut data.shutdown_signal,
            rx,
        )
        .await;
    }

    if let Some(pg_listener) = data.pg_listener.as_mut() {
        return next_signal_with_listener(
            &mut data.interval,
            pg_listener,
            &mut data.shutdown_signal,
        )
        .await;
    }

    next_signal_polling_only(&mut data.interval, &mut data.shutdown_signal).await
}

async fn next_signal_with_local_queue(
    interval: &mut runtime::Interval,
    pg_listener: Option<&mut NotificationStream>,
    shutdown_signal: &mut ShutdownSignal,
    rx: &runtime::Receiver<()>,
) -> NextSignal {
    if let Some(pg_listener) = pg_listener {
        let interval = interval.tick().fuse();
        let pg_listener = pg_listener.next().fuse();
        let internal = rx.recv().fuse();
        let shutdown = shutdown_signal.fuse();
        futures::pin_mut!(interval, pg_listener, internal, shutdown);

        futures::select_biased! {
            _ = shutdown => NextSignal::Shutdown,
            _ = interval => NextSignal::Source(JobSignalSource::Polling),
            res = pg_listener => pg_listener_result_to_signal(res),
            res = internal => {
                if res.is_ok() {
                    NextSignal::Source(JobSignalSource::LocalQueue)
                } else {
                    NextSignal::LocalQueueClosed
                }
            },
        }
    } else {
        let interval = interval.tick().fuse();
        let internal = rx.recv().fuse();
        let shutdown = shutdown_signal.fuse();
        futures::pin_mut!(interval, internal, shutdown);

        futures::select_biased! {
            _ = shutdown => NextSignal::Shutdown,
            _ = interval => NextSignal::Source(JobSignalSource::Polling),
            res = internal => {
                if res.is_ok() {
                    NextSignal::Source(JobSignalSource::LocalQueue)
                } else {
                    NextSignal::LocalQueueClosed
                }
            },
        }
    }
}

async fn next_signal_with_listener(
    interval: &mut runtime::Interval,
    pg_listener: &mut NotificationStream,
    shutdown_signal: &mut ShutdownSignal,
) -> NextSignal {
    let interval = interval.tick().fuse();
    let pg_listener = pg_listener.next().fuse();
    let shutdown = shutdown_signal.fuse();
    futures::pin_mut!(interval, pg_listener, shutdown);

    futures::select_biased! {
        _ = shutdown => NextSignal::Shutdown,
        _ = interval => NextSignal::Source(JobSignalSource::Polling),
        res = pg_listener => pg_listener_result_to_signal(res),
    }
}

async fn next_signal_polling_only(
    interval: &mut runtime::Interval,
    shutdown_signal: &mut ShutdownSignal,
) -> NextSignal {
    let interval = interval.tick().fuse();
    let shutdown = shutdown_signal.fuse();
    futures::pin_mut!(interval, shutdown);

    futures::select_biased! {
        _ = shutdown => NextSignal::Shutdown,
        _ = interval => NextSignal::Source(JobSignalSource::Polling),
    }
}

fn pg_listener_result_to_signal(
    result: Option<std::result::Result<Notification, DbError>>,
) -> NextSignal {
    match result {
        Some(Ok(_)) => NextSignal::Source(JobSignalSource::Notification),
        Some(Err(error)) => {
            warn!(
                ?error,
                "PostgreSQL notification listener failed; falling back to polling"
            );
            NextSignal::NotificationListenerClosed
        }
        None => {
            warn!("PostgreSQL notification listener closed; falling back to polling");
            NextSignal::NotificationListenerClosed
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::stream;

    use super::*;

    fn pending_shutdown_signal() -> ShutdownSignal {
        futures::future::pending::<()>().boxed().shared()
    }

    fn ready_shutdown_signal() -> ShutdownSignal {
        futures::future::ready(()).boxed().shared()
    }

    async fn delayed_interval() -> runtime::Interval {
        let mut interval = runtime::interval(Duration::from_secs(60));
        interval.tick().await;
        interval
    }

    #[tokio::test]
    async fn polling_only_returns_shutdown_or_polling() {
        let mut interval = runtime::interval(Duration::from_secs(60));
        let mut shutdown = ready_shutdown_signal();
        assert!(matches!(
            next_signal_polling_only(&mut interval, &mut shutdown).await,
            NextSignal::Shutdown
        ));

        let mut interval = runtime::interval(Duration::from_millis(1));
        let mut shutdown = pending_shutdown_signal();
        assert!(matches!(
            next_signal_polling_only(&mut interval, &mut shutdown).await,
            NextSignal::Source(JobSignalSource::Polling)
        ));
    }

    #[tokio::test]
    async fn local_queue_signal_returns_local_queue_or_closed() {
        let mut interval = delayed_interval().await;
        let mut shutdown = pending_shutdown_signal();
        let (tx, rx) = runtime::channel(1);
        tx.send(()).await.expect("local queue send should succeed");

        assert!(matches!(
            next_signal_with_local_queue(&mut interval, None, &mut shutdown, &rx).await,
            NextSignal::Source(JobSignalSource::LocalQueue)
        ));

        drop(tx);
        assert!(matches!(
            next_signal_with_local_queue(&mut interval, None, &mut shutdown, &rx).await,
            NextSignal::LocalQueueClosed
        ));
    }

    #[tokio::test]
    async fn local_queue_signal_can_return_polling_without_listener() {
        let mut interval = runtime::interval(Duration::from_millis(1));
        let mut shutdown = pending_shutdown_signal();
        let (_tx, rx) = runtime::channel(1);

        assert!(matches!(
            next_signal_with_local_queue(&mut interval, None, &mut shutdown, &rx).await,
            NextSignal::Source(JobSignalSource::Polling)
        ));
    }

    #[tokio::test]
    async fn local_queue_signal_with_listener_still_accepts_local_queue_source() {
        let mut interval = delayed_interval().await;
        let mut shutdown = pending_shutdown_signal();
        let (tx, rx) = runtime::channel(1);
        let mut listener: NotificationStream = Box::pin(stream::pending());
        tx.send(()).await.expect("local queue send should succeed");

        assert!(matches!(
            next_signal_with_local_queue(&mut interval, Some(&mut listener), &mut shutdown, &rx)
                .await,
            NextSignal::Source(JobSignalSource::LocalQueue)
        ));
    }

    #[tokio::test]
    async fn local_queue_signal_with_listener_reports_closed_local_queue() {
        let mut interval = delayed_interval().await;
        let mut shutdown = pending_shutdown_signal();
        let (tx, rx) = runtime::channel(1);
        let mut listener: NotificationStream = Box::pin(stream::pending());
        drop(tx);

        assert!(matches!(
            next_signal_with_local_queue(&mut interval, Some(&mut listener), &mut shutdown, &rx)
                .await,
            NextSignal::LocalQueueClosed
        ));
    }

    #[tokio::test]
    async fn listener_signal_maps_notifications_and_errors() {
        let mut interval = delayed_interval().await;
        let mut shutdown = pending_shutdown_signal();
        let notification = Notification {
            channel: "jobs:insert".to_string(),
            payload: String::new(),
        };
        let mut listener: NotificationStream = Box::pin(stream::once(async { Ok(notification) }));

        assert!(matches!(
            next_signal_with_listener(&mut interval, &mut listener, &mut shutdown).await,
            NextSignal::Source(JobSignalSource::Notification)
        ));

        let mut interval = delayed_interval().await;
        let mut shutdown = pending_shutdown_signal();
        let mut listener: NotificationStream =
            Box::pin(stream::once(async { Err(DbError::new("listener failed")) }));

        assert!(matches!(
            next_signal_with_listener(&mut interval, &mut listener, &mut shutdown).await,
            NextSignal::NotificationListenerClosed
        ));

        let mut interval = delayed_interval().await;
        let mut shutdown = pending_shutdown_signal();
        let mut listener: NotificationStream =
            Box::pin(stream::empty::<std::result::Result<Notification, DbError>>());

        assert!(matches!(
            next_signal_with_listener(&mut interval, &mut listener, &mut shutdown).await,
            NextSignal::NotificationListenerClosed
        ));
    }

    #[test]
    fn maps_raw_listener_results_to_signals() {
        let notification = Notification {
            channel: "jobs:insert".to_string(),
            payload: String::new(),
        };

        assert!(matches!(
            pg_listener_result_to_signal(Some(Ok(notification))),
            NextSignal::Source(JobSignalSource::Notification)
        ));
        assert!(matches!(
            pg_listener_result_to_signal(Some(Err(DbError::new("listener failed")))),
            NextSignal::NotificationListenerClosed
        ));
        assert!(matches!(
            pg_listener_result_to_signal(None),
            NextSignal::NotificationListenerClosed
        ));
    }
}
