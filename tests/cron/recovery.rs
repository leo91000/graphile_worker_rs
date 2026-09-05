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
