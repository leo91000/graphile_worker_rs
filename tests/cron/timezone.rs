use std::{collections::HashMap, process::Command};

use graphile_worker_crontab_runner::{clock::SystemClock, Clock, CronMatcher};
use time::{format_description::well_known::Rfc3339, macros::datetime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::*;

#[derive(Clone)]
struct ObservedClock {
    clock: Arc<MockClock>,
    sleeps: UnboundedSender<OffsetDateTime>,
}

impl Clock for ObservedClock {
    fn now(&self) -> OffsetDateTime {
        self.clock.now()
    }

    async fn sleep_until(&self, datetime: OffsetDateTime) {
        self.sleeps.send(datetime).unwrap();
        self.clock.sleep_until(datetime).await;
    }

    fn local_time(&self, datetime: OffsetDateTime) -> OffsetDateTime {
        SystemClock.local_time(datetime)
    }
}

async fn next_sleep(receiver: &mut UnboundedReceiver<OffsetDateTime>) -> OffsetDateTime {
    tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .expect("Cron did not reach its next sleep")
        .expect("Cron stopped unexpectedly")
}

fn in_timezone(test_name: &str, timezone: &str) -> bool {
    // SystemClock uses the process timezone. Isolate the regression from other tests.
    if std::env::var("TZ").as_deref() != Ok(timezone) {
        let output = Command::new(std::env::current_exe().unwrap())
            .arg(format!("timezone::{test_name}"))
            .args(["--exact", "--nocapture", "--test-threads=1"])
            .env("TZ", timezone)
            .output()
            .expect("Could not start the timezone regression test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed;"),
            "Timezone regression failed in {timezone}.\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }
    true
}

async fn check_timezone(test_name: &str, use_local_time: bool, backfill: bool) {
    if !in_timezone(test_name, "America/Toronto") {
        return;
    }

    let utc_midnight = datetime!(2026-10-01 00:00 UTC);
    let local_midnight = datetime!(2026-10-01 00:00 -04:00);
    assert_eq!(
        SystemClock.local_time(local_midnight).offset(),
        local_midnight.offset()
    );
    let expected_midnight = if use_local_time {
        local_midnight
    } else {
        utc_midnight
    };

    for scheduled_at in [utc_midnight, local_midnight] {
        with_test_db(move |test_db| async move {
            test_db.worker_utils().migrate().await.unwrap();

            if backfill {
                query(
                    "insert into graphile_worker._private_known_crontabs \
                     (identifier, known_since) values ('midnight', $1)",
                )
                .bind(scheduled_at - Duration::minutes(1))
                .execute(&test_db.test_pool)
                .await
                .unwrap();
            }

            let start = if backfill {
                scheduled_at + Duration::seconds(30)
            } else {
                scheduled_at - Duration::seconds(30)
            };
            let clock = Arc::new(MockClock::new(SystemClock.local_time(start)));
            let (sleeps, mut receiver) = unbounded_channel();
            let observed_clock = ObservedClock {
                clock: clock.clone(),
                sleeps,
            };
            let (shutdown_signal, shutdown_notify) = create_shutdown_signal();
            let database = test_db.database.clone();
            let runner_handle = spawn_local(async move {
                let crontabs = parse_crontab("0 0 1 10 4 midnight ?fill=1m {}").unwrap();
                CronRunner::new(
                    &database,
                    "graphile_worker",
                    &crontabs,
                    &HookRegistry::default(),
                )
                .use_local_time(use_local_time)
                .with_clock(observed_clock)
                .run(shutdown_signal)
                .await
            });

            let first_tick = next_sleep(&mut receiver).await;
            if backfill {
                assert_eq!(first_tick, scheduled_at + Duration::minutes(1));
            } else {
                assert_eq!(first_tick, scheduled_at);
                clock.set_time(SystemClock.local_time(scheduled_at + Duration::seconds(1)));
                let following_tick = next_sleep(&mut receiver).await;
                assert_eq!(following_tick - first_tick, Duration::minutes(1));
            }

            shutdown_notify.notify_one();
            tokio::time::timeout(std::time::Duration::from_secs(5), runner_handle)
                .await
                .expect("Cron shutdown timed out")
                .expect("Cron runner panicked")
                .expect("Cron runner failed");

            let jobs = test_db.get_jobs().await;
            let should_schedule = scheduled_at == expected_midnight;
            assert_eq!(jobs.len(), usize::from(should_schedule));
            let known_crontabs = test_db.get_known_crontabs().await;
            assert_eq!(known_crontabs.len(), 1);
            if should_schedule {
                assert_eq!(jobs[0].task_identifier, "midnight");
                assert_eq!(jobs[0].run_at, scheduled_at);
                assert_eq!(jobs[0].payload["_cron"]["backfilled"], backfill);
                let payload_time = OffsetDateTime::parse(
                    jobs[0].payload["_cron"]["ts"].as_str().unwrap(),
                    &Rfc3339,
                )
                .unwrap();
                assert_eq!(payload_time, scheduled_at);
                assert_eq!(payload_time.offset(), local_midnight.offset());
                assert_eq!(known_crontabs[0].last_execution(), &Some(scheduled_at));
            } else {
                assert!(known_crontabs[0].last_execution().is_none());
            }
        })
        .await;
    }
}

#[tokio::test]
async fn utc_cron_matching_uses_utc_calendar_fields() {
    check_timezone("utc_cron_matching_uses_utc_calendar_fields", false, false).await;
}

#[tokio::test]
async fn local_cron_matching_uses_local_calendar_fields() {
    check_timezone(
        "local_cron_matching_uses_local_calendar_fields",
        true,
        false,
    )
    .await;
}

#[tokio::test]
async fn utc_backfill_matching_uses_utc_calendar_fields() {
    check_timezone(
        "utc_backfill_matching_uses_utc_calendar_fields",
        false,
        true,
    )
    .await;
}

#[tokio::test]
async fn local_backfill_matching_uses_local_calendar_fields() {
    check_timezone(
        "local_backfill_matching_uses_local_calendar_fields",
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn local_cron_uses_the_offset_at_each_tick_across_daylight_saving_time() {
    if !in_timezone(
        "local_cron_uses_the_offset_at_each_tick_across_daylight_saving_time",
        "America/Toronto",
    ) {
        return;
    }

    with_test_db(|test_db| async move {
        test_db.worker_utils().migrate().await.unwrap();

        let start = datetime!(2026-03-08 01:59:30 -05:00);
        let first_tick = datetime!(2026-03-08 03:00 -04:00);
        let clock = Arc::new(MockClock::new(start));
        let (sleeps, mut receiver) = unbounded_channel();
        let observed_clock = ObservedClock {
            clock: clock.clone(),
            sleeps,
        };
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();
        let database = test_db.database.clone();
        let runner_handle = spawn_local(async move {
            let crontabs = parse_crontab("0 2 * * * skipped {}\n0 3 * * * scheduled {}").unwrap();
            CronRunner::new(
                &database,
                "graphile_worker",
                &crontabs,
                &HookRegistry::default(),
            )
            .use_local_time(true)
            .with_clock(observed_clock)
            .run(shutdown_signal)
            .await
        });

        assert_eq!(next_sleep(&mut receiver).await, first_tick);
        clock.set_time(first_tick + Duration::minutes(1));
        assert_eq!(
            next_sleep(&mut receiver).await,
            first_tick + Duration::minutes(1)
        );
        assert_eq!(
            next_sleep(&mut receiver).await,
            first_tick + Duration::minutes(2)
        );

        shutdown_notify.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(5), runner_handle)
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task_identifier, "scheduled");
        assert_eq!(jobs[0].run_at, first_tick);
        assert_eq!(jobs[0].payload["_cron"]["ts"], "2026-03-08T03:00:00-04:00");
    })
    .await;
}

#[tokio::test]
async fn local_backfill_preserves_minutes_across_subminute_offset_changes() {
    if !in_timezone(
        "local_backfill_preserves_minutes_across_subminute_offset_changes",
        "Europe/Paris",
    ) {
        return;
    }

    with_test_db(|test_db| async move {
        test_db.worker_utils().migrate().await.unwrap();
        query(
            "insert into graphile_worker._private_known_crontabs \
             (identifier, known_since) values ('historical', $1)",
        )
        .bind(datetime!(1911-03-10 23:00 UTC))
        .execute(&test_db.test_pool)
        .await
        .unwrap();

        let start = datetime!(1911-03-11 00:05 UTC);
        let (sleeps, mut receiver) = unbounded_channel();
        let observed_clock = ObservedClock {
            clock: Arc::new(MockClock::new(SystemClock.local_time(start))),
            sleeps,
        };
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();
        let database = test_db.database.clone();
        let runner_handle = spawn_local(async move {
            let crontabs = parse_crontab("* * * * * historical ?fill=30m {}").unwrap();
            let matcher: CronMatcher = Arc::new(|at| {
                at.hour() == 23 && matches!(at.minute(), 45 | 51) && at.second() == 0
            });
            let matchers = HashMap::from([(0, matcher)]);
            CronRunner::new(
                &database,
                "graphile_worker",
                &crontabs,
                &HookRegistry::default(),
            )
            .use_local_time(true)
            .with_matchers(&matchers)
            .with_clock(observed_clock)
            .run(shutdown_signal)
            .await
        });

        assert_eq!(
            next_sleep(&mut receiver).await,
            start + Duration::minutes(1)
        );
        shutdown_notify.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(5), runner_handle)
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        let mut run_at = test_db
            .get_jobs()
            .await
            .into_iter()
            .map(|job| job.run_at)
            .collect::<Vec<_>>();
        run_at.sort_unstable();
        assert_eq!(
            run_at,
            [
                datetime!(1911-03-10 23:45 +00:09:21),
                datetime!(1911-03-10 23:51 +00:09:21),
                datetime!(1911-03-10 23:51 UTC),
            ],
        );
    })
    .await;
}

#[tokio::test]
async fn local_cron_accepts_an_exact_utc_wake_with_a_subminute_offset() {
    if !in_timezone(
        "local_cron_accepts_an_exact_utc_wake_with_a_subminute_offset",
        "XXX-5:30:20",
    ) {
        return;
    }

    with_test_db(|test_db| async move {
        test_db.worker_utils().migrate().await.unwrap();

        let start = datetime!(2026-01-01 01:29:30 UTC);
        let tick = datetime!(2026-01-01 01:29:40 UTC);
        let clock = Arc::new(MockClock::new(start));
        let (sleeps, mut receiver) = unbounded_channel();
        let observed_clock = ObservedClock {
            clock: clock.clone(),
            sleeps,
        };
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();
        let database = test_db.database.clone();
        let runner_handle = spawn_local(async move {
            let crontabs = parse_crontab("0 7 * * * subminute {}").unwrap();
            CronRunner::new(
                &database,
                "graphile_worker",
                &crontabs,
                &HookRegistry::default(),
            )
            .use_local_time(true)
            .with_clock(observed_clock)
            .run(shutdown_signal)
            .await
        });

        assert_eq!(next_sleep(&mut receiver).await, tick);
        clock.set_time(tick);
        assert_eq!(next_sleep(&mut receiver).await, tick + Duration::minutes(1));

        shutdown_notify.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(5), runner_handle)
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].run_at, tick);
        assert_eq!(
            jobs[0].payload["_cron"]["ts"],
            "2026-01-01T07:00:00+05:30:20"
        );
    })
    .await;
}
