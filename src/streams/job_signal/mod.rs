mod select;
mod state;

use std::time::{Duration, Instant};

use futures::{stream, FutureExt, Stream};
use graphile_worker_database::{Database, Schema};
use graphile_worker_migrations::{check_running_schema, MigrateError, MIGRATION_CHANNEL};
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::warn;

use crate::local_queue::LocalQueueSignalReceiver;
use crate::runner::WorkerRuntimeError;
use state::{JobSignalStreamData, NextSignal};

const MIN_SCHEMA_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Indicates why a worker should check for jobs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum JobSignalSource {
    /// Job processing was triggered by a database poll, including a local queue-release hint.
    Polling,
    /// Job processing was triggered by a PostgreSQL notification.
    Notification,
    /// Job processing came from a one-time run request.
    RunOnce,
    /// Job processing was triggered by LocalQueue activity or a queue-release pulse.
    LocalQueue,
}

pub(crate) struct JobSignalStreamConfig {
    database: Database,
    poll_interval: Duration,
    use_notification_system: bool,
    shutdown_signal: ShutdownSignal,
    local_queue_rx: Option<LocalQueueSignalReceiver>,
    schema: Option<Schema>,
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
            schema: None,
        }
    }

    pub(crate) fn with_schema(mut self, schema: Schema) -> Self {
        self.schema = Some(schema);
        self
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
) -> Result<impl Stream<Item = Result<JobSignalSource, WorkerRuntimeError>>, WorkerRuntimeError> {
    let interval = graphile_worker_runtime::interval(config.poll_interval);

    let pg_listener = if config.use_notification_system {
        let channels: &[&str] = if config.schema.is_some() {
            &[MIGRATION_CHANNEL]
        } else {
            &[]
        };
        config
            .database
            .listen_with_channels("jobs:insert", channels)
            .await
            .map_err(crate::errors::GraphileWorkerError::from)?
    } else {
        None
    };

    let stream_data = JobSignalStreamData::new(
        interval,
        pg_listener,
        config.shutdown_signal,
        config.local_queue_rx,
    );
    let migration_watch = config.schema.map(|schema| MigrationWatch {
        database: config.database,
        schema,
        next_check: Instant::now(),
    });
    let stream = stream::unfold(
        (stream_data, migration_watch),
        |(mut data, mut watch)| async {
            loop {
                let (source, check_now) = match select::next_signal(&mut data).await {
                    NextSignal::Source(source) => (Some(source), false),
                    NextSignal::LocalQueueClosed => {
                        warn!("Job signal stream LocalQueue channel closed");
                        return None;
                    }
                    NextSignal::NotificationListenerClosed => {
                        data.pg_listener = None;
                        (Some(JobSignalSource::Polling), true)
                    }
                    NextSignal::CheckMigration => (None, true),
                    NextSignal::IgnoreNotification => continue,
                    NextSignal::Shutdown => return None,
                };

                if let Some(migration_watch) = &mut watch {
                    if check_now
                        || (source == Some(JobSignalSource::Polling)
                            && Instant::now() >= migration_watch.next_check)
                    {
                        let result = {
                            let check = migration_watch.check().fuse();
                            let shutdown = (&mut data.shutdown_signal).fuse();
                            futures::pin_mut!(check, shutdown);
                            futures::select_biased! {
                                _ = shutdown => return None,
                                result = check => result,
                            }
                        };
                        if let Err(error) = result {
                            return Some((Err(error), (data, watch)));
                        }
                    }
                }
                if let Some(source) = source {
                    return Some((Ok(source), (data, watch)));
                }
            }
        },
    );

    Ok(stream)
}

struct MigrationWatch {
    database: Database,
    schema: Schema,
    next_check: Instant,
}

impl MigrationWatch {
    async fn check(&mut self) -> Result<(), WorkerRuntimeError> {
        self.next_check = Instant::now() + MIN_SCHEMA_CHECK_INTERVAL;
        match check_running_schema(&self.database, &self.schema).await {
            Err(error @ MigrateError::IncompatbleRevision { .. }) => {
                Err(WorkerRuntimeError::IncompatibleSchema(error))
            }
            Err(error) => {
                warn!(%error, "Unable to verify running worker schema");
                Ok(())
            }
            Ok(()) => Ok(()),
        }
    }
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
        assert!(matches!(
            stream.next().await,
            Some(Ok(JobSignalSource::Polling))
        ));
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
        assert!(matches!(
            stream.next().await,
            Some(Ok(JobSignalSource::Polling))
        ));
        assert!(matches!(
            stream.next().await,
            Some(Ok(JobSignalSource::Polling))
        ));
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
        assert!(matches!(
            stream.next().await,
            Some(Ok(JobSignalSource::Polling))
        ));
        assert!(stream.next().await.is_none());
    }
}
