mod select;
mod state;

use std::time::Duration;

use futures::{stream, Stream};
use graphile_worker_database::Database;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::warn;

use crate::errors::Result;
use crate::local_queue::LocalQueueSignalReceiver;
use state::{JobSignalStreamData, NextSignal};

/// Indicates why a worker should check for jobs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum JobSignalSource {
    /// Job processing was triggered by a regular polling interval.
    Polling,
    /// Job processing was triggered by a PostgreSQL notification.
    Notification,
    /// Job processing came from a one-time run request.
    RunOnce,
    /// Job processing was triggered by LocalQueue cache activity.
    LocalQueue,
}

pub(crate) struct JobSignalStreamConfig {
    database: Database,
    poll_interval: Duration,
    use_notification_system: bool,
    shutdown_signal: ShutdownSignal,
    local_queue_rx: Option<LocalQueueSignalReceiver>,
}

impl JobSignalStreamConfig {
    pub(crate) fn new(
        database: Database,
        poll_interval: Duration,
        use_notification_system: bool,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        Self {
            database,
            poll_interval,
            use_notification_system,
            shutdown_signal,
            local_queue_rx: None,
        }
    }

    pub(crate) fn with_local_queue(mut self, local_queue_rx: LocalQueueSignalReceiver) -> Self {
        self.local_queue_rx = Some(local_queue_rx);
        self
    }
}

/// Creates a stream that yields job processing signals from polling, optional
/// PostgreSQL notifications, optional LocalQueue signals, and shutdown.
///
/// Fanout is intentionally not handled here. The dispatcher owns worker fanout
/// and coalesces signals under backpressure so notification streams keep being
/// polled even when all workers are busy.
pub(crate) async fn job_signal_stream(
    config: JobSignalStreamConfig,
) -> Result<impl Stream<Item = JobSignalSource>> {
    let interval = graphile_worker_runtime::interval(config.poll_interval);

    let pg_listener = if config.use_notification_system {
        config.database.listen("jobs:insert").await?
    } else {
        None
    };

    let stream_data = JobSignalStreamData::new(
        interval,
        pg_listener,
        config.shutdown_signal,
        config.local_queue_rx,
    );
    let stream = stream::unfold(stream_data, |mut data| async {
        match select::next_signal(&mut data).await {
            NextSignal::Source(source) => Some((source, data)),
            NextSignal::LocalQueueClosed => {
                warn!("Job signal stream LocalQueue channel closed");
                None
            }
            NextSignal::NotificationListenerClosed => {
                data.pg_listener = None;
                Some((JobSignalSource::Polling, data))
            }
            NextSignal::Shutdown => None,
        }
    });

    Ok(stream)
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::fmt;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use futures::{stream, FutureExt, StreamExt};
    use graphile_worker_database::{
        BoxFuture, Database, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
        Notification, NotificationStream,
    };

    use super::*;

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
        ) -> BoxFuture<'a, std::result::Result<u64, DbError>> {
            Box::pin(async { Ok(0) })
        }

        fn fetch_all<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, std::result::Result<Vec<DbRow>, DbError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    impl DatabaseDriver for ListenCountingDriver {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn begin<'a>(&'a self) -> BoxFuture<'a, std::result::Result<DbTransaction, DbError>> {
            Box::pin(async { Err(DbError::new("transactions are unavailable")) })
        }

        fn listen<'a>(
            &'a self,
            _channel: &'a str,
        ) -> BoxFuture<'a, std::result::Result<Option<NotificationStream>, DbError>> {
            self.listen_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err(DbError::new("listen should not be called")) })
        }
    }

    #[derive(Clone, Default)]
    struct ClosingListenerDriver;

    impl fmt::Debug for ClosingListenerDriver {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ClosingListenerDriver").finish()
        }
    }

    impl DbExecutor for ClosingListenerDriver {
        fn execute<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, std::result::Result<u64, DbError>> {
            Box::pin(async { Ok(0) })
        }

        fn fetch_all<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, std::result::Result<Vec<DbRow>, DbError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    impl DatabaseDriver for ClosingListenerDriver {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn begin<'a>(&'a self) -> BoxFuture<'a, std::result::Result<DbTransaction, DbError>> {
            Box::pin(async { Err(DbError::new("transactions are unavailable")) })
        }

        fn listen<'a>(
            &'a self,
            _channel: &'a str,
        ) -> BoxFuture<'a, std::result::Result<Option<NotificationStream>, DbError>> {
            let listener: NotificationStream =
                Box::pin(stream::empty::<std::result::Result<Notification, DbError>>());
            Box::pin(async { Ok(Some(listener)) })
        }
    }

    fn pending_shutdown_signal() -> ShutdownSignal {
        futures::future::pending::<()>().boxed().shared()
    }

    #[tokio::test]
    async fn disabled_notification_delivery_skips_listener() {
        let driver = ListenCountingDriver::default();
        let database = Database::new(driver.clone());

        let stream = job_signal_stream(JobSignalStreamConfig::new(
            database,
            Duration::from_millis(1),
            false,
            pending_shutdown_signal(),
        ))
        .await
        .expect("stream should initialize");

        futures::pin_mut!(stream);
        assert_eq!(stream.next().await, Some(JobSignalSource::Polling));
        assert_eq!(driver.listen_calls(), 0);
    }

    #[tokio::test]
    async fn enabled_notification_delivery_opens_listener() {
        let driver = ListenCountingDriver::default();
        let database = Database::new(driver.clone());

        let result = job_signal_stream(JobSignalStreamConfig::new(
            database,
            Duration::from_millis(1),
            true,
            pending_shutdown_signal(),
        ))
        .await;

        assert!(result.is_err());
        assert_eq!(driver.listen_calls(), 1);
    }

    #[tokio::test]
    async fn listen_counting_driver_contract_is_exercised() {
        let driver = ListenCountingDriver::default();

        assert!(format!("{driver:?}").contains("ListenCountingDriver"));
        assert_eq!(driver.execute("", DbParams::new()).await.unwrap(), 0);
        assert!(driver
            .fetch_all("", DbParams::new())
            .await
            .unwrap()
            .is_empty());
        assert!(driver.as_any().is::<ListenCountingDriver>());
        assert!(driver.begin().await.is_err());
    }

    #[tokio::test]
    async fn closing_listener_driver_contract_is_exercised() {
        let driver = ClosingListenerDriver;

        assert!(format!("{driver:?}").contains("ClosingListenerDriver"));
        assert_eq!(driver.execute("", DbParams::new()).await.unwrap(), 0);
        assert!(driver
            .fetch_all("", DbParams::new())
            .await
            .unwrap()
            .is_empty());
        assert!(driver.as_any().is::<ClosingListenerDriver>());
        assert!(driver.begin().await.is_err());
    }

    #[tokio::test]
    async fn stream_falls_back_to_polling_when_listener_closes() {
        let database = Database::new(ClosingListenerDriver);

        let stream = job_signal_stream(JobSignalStreamConfig::new(
            database,
            Duration::from_secs(60),
            true,
            pending_shutdown_signal(),
        ))
        .await
        .expect("stream should initialize");

        futures::pin_mut!(stream);
        assert_eq!(stream.next().await, Some(JobSignalSource::Polling));
        assert_eq!(stream.next().await, Some(JobSignalSource::Polling));
    }

    #[tokio::test]
    async fn stream_ends_when_local_queue_signal_channel_closes() {
        let database = Database::new(ListenCountingDriver::default());
        let (tx, rx) = graphile_worker_runtime::channel(1);
        drop(tx);

        let stream = job_signal_stream(
            JobSignalStreamConfig::new(
                database,
                Duration::from_secs(60),
                false,
                pending_shutdown_signal(),
            )
            .with_local_queue(rx),
        )
        .await
        .expect("stream should initialize");

        futures::pin_mut!(stream);
        assert_eq!(stream.next().await, Some(JobSignalSource::Polling));
        assert_eq!(stream.next().await, None);
    }
}
