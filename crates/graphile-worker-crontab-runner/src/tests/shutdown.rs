use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use futures::FutureExt;
use graphile_worker_crontab_types::{Crontab, CrontabFill, CrontabTimer};
use graphile_worker_database::{BoxFuture, DbCell, DbError, DbExecutor, DbParams, DbRow};
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_shutdown_signal::ShutdownSignal;
use time::{macros::datetime, Duration, OffsetDateTime};
use tokio::sync::Notify;

use crate::{Clock, CronRunner};

struct ReadyExecutor {
    known: Vec<DbRow>,
    queries: Arc<AtomicUsize>,
    shutdown_on_execute: Option<Arc<Notify>>,
}

impl DbExecutor for ReadyExecutor {
    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            // Bound the old busy loop so a regression fails instead of hanging.
            assert!(
                self.queries.fetch_add(1, Ordering::SeqCst) < 32,
                "Cron did not yield to shutdown"
            );
            if let Some(shutdown) = &self.shutdown_on_execute {
                shutdown.notify_one();
            }
            Ok(1)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move { Ok(self.known.clone()) })
    }
}

struct WakingClock {
    advance: Duration,
    now: Mutex<OffsetDateTime>,
    sleeps: Arc<AtomicUsize>,
    shutdown: Arc<Notify>,
}

impl Clock for WakingClock {
    fn now(&self) -> OffsetDateTime {
        *self.now.lock().unwrap()
    }

    async fn sleep_until(&self, _datetime: OffsetDateTime) {
        assert!(
            self.sleeps.fetch_add(1, Ordering::SeqCst) < 32,
            "Catch-up did not yield"
        );
        *self.now.lock().unwrap() += self.advance;
        self.shutdown.notify_one();
    }
}

fn shutdown_signal(notify: Arc<Notify>) -> ShutdownSignal {
    async move { notify.notified().await }.boxed().shared()
}

#[tokio::test]
async fn shutdown_interrupts_immediate_clock_wakes() {
    // An early wake should perform registration only. A late wake can also schedule its first tick.
    for (advance, expected_queries) in [(Duration::ZERO, 1), (Duration::days(1), 2)] {
        let shutdown = Arc::new(Notify::new());
        let sleeps = Arc::new(AtomicUsize::new(0));
        let queries = Arc::new(AtomicUsize::new(0));
        let clock = WakingClock {
            advance,
            now: Mutex::new(datetime!(2026-01-15 10:00:30 UTC)),
            sleeps: sleeps.clone(),
            shutdown: shutdown.clone(),
        };
        let executor = ReadyExecutor {
            known: vec![],
            queries: queries.clone(),
            shutdown_on_execute: None,
        };
        let crontabs = [Crontab::new(CrontabTimer::every_minute(), "tick")];
        let hooks = HookRegistry::default();

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            CronRunner::new(&executor, "graphile_worker", &crontabs, &hooks)
                .with_clock(clock)
                .run(shutdown_signal(shutdown)),
        )
        .await
        .expect("Shutdown timed out")
        .unwrap();

        assert_eq!(sleeps.load(Ordering::SeqCst), 1);
        assert_eq!(queries.load(Ordering::SeqCst), expected_queries);
    }
}

#[tokio::test]
async fn shutdown_interrupts_backfill_with_immediately_ready_database() {
    let now = datetime!(2026-01-15 10:00:30 UTC);
    let shutdown = Arc::new(Notify::new());
    let queries = Arc::new(AtomicUsize::new(0));
    let executor = ReadyExecutor {
        known: vec![DbRow::new(
            [
                ("identifier".into(), DbCell::Text("backfill".into())),
                (
                    "known_since".into(),
                    DbCell::TimestampTz(now - Duration::days(2)),
                ),
                ("last_execution".into(), DbCell::Null),
            ]
            .into(),
        )],
        queries: queries.clone(),
        shutdown_on_execute: Some(shutdown.clone()),
    };
    let mut crontab = Crontab::new(CrontabTimer::every_minute(), "backfill");
    crontab.options.fill = Some(CrontabFill::days(1));
    let crontabs = [crontab];
    let hooks = HookRegistry::default();
    let clock = crate::MockClock::new(now);

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        CronRunner::new(&executor, "graphile_worker", &crontabs, &hooks)
            .with_clock(clock)
            .run(shutdown_signal(shutdown)),
    )
    .await
    .expect("Shutdown timed out")
    .unwrap();

    assert_eq!(
        queries.load(Ordering::SeqCst),
        1,
        "Shutdown must interrupt the remaining backfill"
    );
}
