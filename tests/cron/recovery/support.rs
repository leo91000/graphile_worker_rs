use std::sync::atomic::{AtomicU8, Ordering};

use chrono::{DateTime, Local, TimeZone};
use graphile_worker_crontab_runner::{Clock, MockClock};
use graphile_worker_database::{BoxFuture, Database, DbError, DbExecutor, DbParams, DbRow};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::super::*;

pub(super) const HEALTHY: u8 = 0;
pub(super) const UNAVAILABLE: u8 = 1;
pub(super) const LOSE_RESPONSE: u8 = 2;
pub(super) const BLOCKED: u8 = 3;

#[derive(Clone)]
pub(super) struct FaultyExecutor {
    database: Database,
    pub mode: Arc<AtomicU8>,
    pub blocked: Arc<Notify>,
}

impl FaultyExecutor {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            mode: Arc::new(AtomicU8::new(HEALTHY)),
            blocked: Arc::new(Notify::new()),
        }
    }

    pub fn set_mode(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }

    async fn before_query(&self) -> Result<(), DbError> {
        match self.mode.load(Ordering::SeqCst) {
            // A persistent error intentionally verifies that retries aren't restricted
            // to a list of transient connection errors.
            UNAVAILABLE => Err(DbError::with_code("injected permission failure", "42501")),
            BLOCKED => {
                self.blocked.notify_one();
                std::future::pending().await
            }
            _ => Ok(()),
        }
    }
}

impl DbExecutor for FaultyExecutor {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            self.before_query().await?;
            let rows = self.database.execute(sql, params).await?;
            if sql.contains("with specs as")
                && self
                    .mode
                    .compare_exchange(
                        LOSE_RESPONSE,
                        UNAVAILABLE,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .is_ok()
            {
                return Err(DbError::new("injected lost response after commit"));
            }
            Ok(rows)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            self.before_query().await?;
            self.database.fetch_all(sql, params).await
        })
    }
}

#[derive(Clone)]
pub(super) struct ObservedClock {
    pub clock: Arc<MockClock>,
    sleeps: UnboundedSender<DateTime<Local>>,
}

impl ObservedClock {
    pub fn new() -> (Self, UnboundedReceiver<DateTime<Local>>) {
        let (sleeps, receiver) = unbounded_channel();
        let start = Local.with_ymd_and_hms(2026, 1, 15, 10, 0, 30).unwrap();
        (
            Self {
                clock: Arc::new(MockClock::new(start)),
                sleeps,
            },
            receiver,
        )
    }
}

impl Clock for ObservedClock {
    fn now(&self) -> DateTime<Local> {
        self.clock.now()
    }

    async fn sleep_until(&self, datetime: DateTime<Local>) {
        self.sleeps.send(datetime).unwrap();
        self.clock.sleep_until(datetime).await;
    }
}

pub(super) async fn next_sleep(
    receiver: &mut UnboundedReceiver<DateTime<Local>>,
) -> DateTime<Local> {
    tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .expect("Cron did not reach its next sleep")
        .expect("Cron stopped unexpectedly")
}

pub(super) fn start_runner(
    executor: FaultyExecutor,
    clock: ObservedClock,
    crontab: &str,
) -> (
    tokio::task::JoinHandle<Result<(), graphile_worker_crontab_runner::ScheduleCronJobError>>,
    Arc<Notify>,
) {
    let crontabs = parse_crontab(crontab).unwrap();
    let (shutdown, notify) = create_shutdown_signal();
    let handle = spawn_local(async move {
        CronRunner::new(
            &executor,
            "graphile_worker",
            &crontabs,
            &HookRegistry::default(),
        )
        .with_clock(clock)
        .run(shutdown)
        .await
    });
    (handle, notify)
}

pub(super) async fn stop_runner(
    handle: tokio::task::JoinHandle<
        Result<(), graphile_worker_crontab_runner::ScheduleCronJobError>,
    >,
    notify: Arc<Notify>,
) {
    notify.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("Shutdown was blocked")
        .expect("Runner panicked")
        .expect("Runner failed");
}
