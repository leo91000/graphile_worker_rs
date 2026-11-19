use chrono::{Duration, Timelike};
use graphile_worker_crontab_runner::Clock;

use super::*;

#[path = "recovery/support.rs"]
mod support;
use support::*;

#[tokio::test]
async fn cron_retries_startup_with_capped_backoff_and_resets_after_a_healthy_tick() {
    crate::helpers::enable_logs().await;
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        let executor = FaultyExecutor::new(db.database.clone());
        executor.set_mode(UNAVAILABLE);
        let (clock, mut sleeps) = ObservedClock::new();
        let (handle, shutdown) = start_runner(executor.clone(), clock.clone(), "* * * * * retry");

        let mut delay = Duration::milliseconds(200);
        for _ in 0..24 {
            let deadline = next_sleep(&mut sleeps).await;
            assert_eq!(deadline - clock.now(), delay);
            clock.clock.set_time(deadline);
            delay = (delay + delay / 2).min(Duration::seconds(60));
        }
        assert_eq!(delay, Duration::seconds(60));
        let deadline = next_sleep(&mut sleeps).await;
        assert_eq!(deadline - clock.now(), Duration::seconds(60));
        executor.set_mode(HEALTHY);
        clock.clock.set_time(deadline);

        let tick = next_sleep(&mut sleeps).await;
        assert!(
            db.get_jobs().await.is_empty(),
            "New entries must not backfill"
        );
        clock.clock.set_time(tick);
        let next_tick = next_sleep(&mut sleeps).await;
        assert_eq!(db.get_jobs().await.len(), 1);

        executor.set_mode(UNAVAILABLE);
        clock.clock.set_time(next_tick);
        let retry = next_sleep(&mut sleeps).await;
        assert_eq!(retry - clock.now(), Duration::milliseconds(200));
        // Do not advance the clock: shutdown must cancel backoff.
        stop_runner(handle, shutdown).await;
    })
    .await;
}

#[tokio::test]
async fn cron_recovery_honors_each_entry_fill_window_without_duplicate_jobs() {
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        let executor = FaultyExecutor::new(db.database.clone());
        let (clock, mut sleeps) = ObservedClock::new();
        let crontab = "* * * * * shared ?id=short&fill=2m {schedule:'short'}\n\
                       * * * * * shared ?id=no_fill {schedule:'no_fill'}\n\
                       * * * * * shared ?id=long&fill=10m {schedule:'long'}";
        let (handle, shutdown) = start_runner(executor.clone(), clock.clone(), crontab);
        let first_tick = next_sleep(&mut sleeps).await;
        let known = db.get_known_crontabs().await;
        assert_eq!(known.len(), 3);
        assert!(known.iter().all(|entry| entry.identifier() != "shared"));
        assert!(db.get_jobs().await.is_empty());

        clock.clock.set_time(first_tick);
        let second_tick = next_sleep(&mut sleeps).await;
        assert_eq!(db.get_jobs().await.len(), 3);
        executor.set_mode(LOSE_RESPONSE);
        clock.clock.set_time(second_tick);
        next_sleep(&mut sleeps).await;
        assert_eq!(
            db.get_jobs().await.len(),
            6,
            "The second tick committed before the response was lost"
        );

        // Recover at 10:05:30. Also lose one response during backfill so that
        // registration and backfill must be repeated safely.
        executor.set_mode(LOSE_RESPONSE);
        clock
            .clock
            .set_time(second_tick + Duration::minutes(3) + Duration::seconds(30));
        let retry = next_sleep(&mut sleeps).await;
        executor.set_mode(HEALTHY);
        clock.clock.set_time(retry);
        let next_tick = next_sleep(&mut sleeps).await;
        assert_eq!(next_tick.minute(), 6);

        let jobs = db.get_jobs().await;
        for (entry, expected_minutes) in [
            ("short", vec![1, 2, 4, 5]),
            ("no_fill", vec![1, 2]),
            ("long", vec![1, 2, 3, 4, 5]),
        ] {
            let mut minutes: Vec<_> = jobs
                .iter()
                .filter(|job| job.payload["schedule"] == entry)
                .map(|job| job.run_at.minute())
                .collect();
            minutes.sort_unstable();
            assert_eq!(minutes, expected_minutes, "{entry}");
        }
        assert_eq!(jobs.len(), 11);
        stop_runner(handle, shutdown).await;

        // A new runner must read the same history and avoid replaying these jobs.
        let (handle, shutdown) = start_runner(executor, clock, crontab);
        assert_eq!(next_sleep(&mut sleeps).await, next_tick);
        assert_eq!(db.get_jobs().await.len(), 11);
        stop_runner(handle, shutdown).await;
    })
    .await;
}

#[tokio::test]
async fn cron_shutdown_interrupts_a_pending_database_operation() {
    with_test_db(|db| async move {
        let executor = FaultyExecutor::new(db.database.clone());
        executor.set_mode(BLOCKED);
        let (clock, _sleeps) = ObservedClock::new();
        let (handle, shutdown) = start_runner(executor.clone(), clock, "* * * * * blocked");
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            executor.blocked.notified(),
        )
        .await
        .expect("Runner did not enter the database operation");
        stop_runner(handle, shutdown).await;
    })
    .await;
}

#[tokio::test]
async fn programmatic_matcher_and_backfill_observer_use_the_same_calendar_as_live_ticks() {
    use graphile_worker::{CronBackfill, CronBackfillContext};
    use graphile_worker_crontab_runner::CronMatcher;

    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        let (clock, mut sleeps) = ObservedClock::new();
        let known_since = clock.now();
        let last_execution = known_since + Duration::seconds(30);
        query("insert into graphile_worker._private_known_crontabs (identifier, known_since, last_execution) values ('even', $1, $2)")
            .bind(known_since)
            .bind(last_execution)
            .execute(&db.test_pool).await.unwrap();
        clock.clock.set_time(known_since + Duration::minutes(5));

        let mut hooks = HookRegistry::default();
        let (observed, mut observations) = tokio::sync::mpsc::unbounded_channel::<CronBackfillContext>();
        hooks.on(CronBackfill, move |context| {
            let observed = observed.clone();
            async move { observed.send(context).unwrap(); }
        });

        // The predicate replaces a timer that could not match any minute under test.
        let crontabs = parse_crontab("59 * * * * shared ?id=even&fill=10m {}\n7 * * * * shared ?id=plain {}").unwrap();
        let even_minute: CronMatcher = Arc::new(|date| date.minute() % 2 == 0);
        let matchers = [(0, even_minute)].into();
        let (shutdown, notify) = create_shutdown_signal();
        let handle = spawn_local({
            let database = db.database.clone();
            let clock = clock.clone();
            async move {
                CronRunner::new(&database, "graphile_worker", &crontabs, &hooks)
                    .with_matchers(&matchers)
                    .with_clock(clock)
                    .use_local_time(true)
                    .run(shutdown)
                    .await
            }
        });
        let first_live_tick = next_sleep(&mut sleeps).await;
        assert_eq!(first_live_tick.minute(), 6);
        for minute in [2, 4] {
            let event = observations.try_recv().unwrap();
            assert_eq!(event.scheduled_at.with_timezone(&Local).minute(), minute);
            assert_eq!(event.entries.len(), 1);
            let entry = &event.entries[0];
            assert_eq!(entry.crontab.identifier(), "even");
            assert_eq!(entry.known_since, known_since);
            assert_eq!(entry.last_execution.unwrap(), last_execution);
        }
        assert!(observations.try_recv().is_err());

        clock.clock.set_time(first_live_tick);
        let second_live_tick = next_sleep(&mut sleeps).await;
        clock.clock.set_time(second_live_tick);
        next_sleep(&mut sleeps).await;

        let mut outcomes: Vec<_> = db.get_jobs().await.into_iter().map(|job| (
            job.run_at.with_timezone(&Local).minute(),
            job.payload["_cron"]["backfilled"].as_bool().unwrap(),
        )).collect();
        outcomes.sort_unstable();
        assert_eq!(outcomes, [(2, true), (4, true), (6, false), (7, false)]);
        stop_runner(handle, notify).await;
    }).await;
}

#[tokio::test]
async fn a_panicking_matcher_retries_through_cron_recovery() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use graphile_worker_crontab_runner::CronMatcher;

    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        let (clock, mut sleeps) = ObservedClock::new();
        let crontabs = parse_crontab("* * * * * retry ?fill=1m").unwrap();
        let fail_once = AtomicBool::new(true);
        let predicate: CronMatcher = Arc::new(move |_| {
            assert!(
                !fail_once.swap(false, Ordering::SeqCst),
                "injected matcher panic"
            );
            true
        });
        let matchers = [(0, predicate)].into();
        let (shutdown, notify) = create_shutdown_signal();
        let handle = spawn_local({
            let database = db.database.clone();
            let clock = clock.clone();
            async move {
                CronRunner::new(
                    &database,
                    "graphile_worker",
                    &crontabs,
                    &HookRegistry::default(),
                )
                .with_matchers(&matchers)
                .with_clock(clock)
                .run(shutdown)
                .await
            }
        });
        let tick = next_sleep(&mut sleeps).await;
        clock.clock.set_time(tick);
        let retry = next_sleep(&mut sleeps).await;
        assert_eq!(retry - tick, Duration::milliseconds(200));
        assert!(db.get_jobs().await.is_empty());

        clock.clock.set_time(retry);
        assert_eq!(next_sleep(&mut sleeps).await, tick + Duration::minutes(1));
        let jobs = db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].run_at, tick);
        stop_runner(handle, notify).await;
    })
    .await;
}
