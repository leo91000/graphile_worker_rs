use futures::{FutureExt, StreamExt};
use graphile_worker_database::{DbError, Notification, NotificationStream};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::warn;

use super::state::{JobSignalStreamData, NextSignal};
use super::StreamSource;

pub(super) async fn next_signal(data: &mut JobSignalStreamData) -> NextSignal {
    if let Some(ref mut rx) = data.internal_rx {
        return next_signal_with_internal(
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

async fn next_signal_with_internal(
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
            _ = interval => NextSignal::Source(StreamSource::Polling),
            res = pg_listener => pg_listener_result_to_signal(res),
            res = internal => {
                if res.is_ok() {
                    NextSignal::Source(StreamSource::Internal)
                } else {
                    NextSignal::InternalClosed
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
            _ = interval => NextSignal::Source(StreamSource::Polling),
            res = internal => {
                if res.is_ok() {
                    NextSignal::Source(StreamSource::Internal)
                } else {
                    NextSignal::InternalClosed
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
        _ = interval => NextSignal::Source(StreamSource::Polling),
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
        _ = interval => NextSignal::Source(StreamSource::Polling),
    }
}

fn pg_listener_result_to_signal(
    result: Option<std::result::Result<Notification, DbError>>,
) -> NextSignal {
    match result {
        Some(Ok(_)) => NextSignal::Source(StreamSource::PgListener),
        Some(Err(error)) => {
            warn!(
                ?error,
                "PostgreSQL notification listener failed; falling back to polling"
            );
            NextSignal::PgListenerClosed
        }
        None => {
            warn!("PostgreSQL notification listener closed; falling back to polling");
            NextSignal::PgListenerClosed
        }
    }
}
