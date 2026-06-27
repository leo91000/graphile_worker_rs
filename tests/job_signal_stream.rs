use std::any::Any;
use std::fmt;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use futures::{FutureExt, StreamExt};
use graphile_worker::{
    streams::{job_signal_stream, StreamSource},
    BoxFuture, Database, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
    NotificationStream, ShutdownSignal,
};

#[derive(Clone, Default)]
struct ListenCountingDriver {
    listen_calls: Arc<AtomicUsize>,
}

impl ListenCountingDriver {
    fn listen_calls(&self) -> usize {
        self.listen_calls.load(Ordering::SeqCst)
    }
}

impl fmt::Debug for ListenCountingDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListenCountingDriver").finish()
    }
}

impl DbExecutor for ListenCountingDriver {
    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async { Ok(0) })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

impl DatabaseDriver for ListenCountingDriver {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn begin<'a>(&'a self) -> BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async { Err(DbError::new("transactions are unavailable")) })
    }

    fn listen<'a>(
        &'a self,
        _channel: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        self.listen_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(DbError::new("listen should not be called")) })
    }
}

fn pending_shutdown_signal() -> ShutdownSignal {
    futures::future::pending::<()>().boxed().shared()
}

#[tokio::test]
async fn disabled_notification_delivery_skips_listener() {
    let driver = ListenCountingDriver::default();
    let database = Database::new(driver.clone());

    let stream = job_signal_stream(
        database,
        Duration::from_millis(1),
        false,
        pending_shutdown_signal(),
        1,
    )
    .await
    .expect("stream should initialize");

    futures::pin_mut!(stream);
    assert_eq!(stream.next().await, Some(StreamSource::Polling));
    assert_eq!(driver.listen_calls(), 0);
}

#[tokio::test]
async fn enabled_notification_delivery_opens_listener() {
    let driver = ListenCountingDriver::default();
    let database = Database::new(driver.clone());

    let result = job_signal_stream(
        database,
        Duration::from_millis(1),
        true,
        pending_shutdown_signal(),
        1,
    )
    .await;

    assert!(result.is_err());
    assert_eq!(driver.listen_calls(), 1);
}
