use chrono::{DateTime, TimeZone, Utc};
use graphile_worker_crontab_runner::Clock;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::*;

#[derive(Clone)]
struct ObservedClock {
    clock: Arc<MockClock>,
    sleeps: UnboundedSender<DateTime<Local>>,
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

async fn next_sleep(receiver: &mut UnboundedReceiver<DateTime<Local>>) -> DateTime<Local> {
    tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .expect("Cron did not reach its next sleep")
        .expect("Cron stopped unexpectedly")
}

async fn check_timezone(use_local_time: bool, backfill: bool) {
    let utc_midnight = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
    let local_midnight = Local.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
    let expected_midnight = if use_local_time {
        local_midnight.with_timezone(&Utc)
    } else {
        utc_midnight
    };

    for scheduled_at in [utc_midnight, local_midnight.with_timezone(&Utc)] {
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
            let clock = Arc::new(MockClock::new(start.with_timezone(&Local)));
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
                assert_eq!(
                    first_tick.with_timezone(&Utc),
                    scheduled_at + Duration::minutes(1)
                );
            } else {
                assert_eq!(first_tick.with_timezone(&Utc), scheduled_at);
                clock.set_time((scheduled_at + Duration::seconds(1)).with_timezone(&Local));
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
                let payload_time: DateTime<chrono::FixedOffset> = jobs[0].payload["_cron"]["ts"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(payload_time.with_timezone(&Utc), scheduled_at);
                assert_eq!(
                    known_crontabs[0].last_execution(),
                    &Some(scheduled_at.with_timezone(&Local))
                );
            } else {
                assert!(known_crontabs[0].last_execution().is_none());
            }
        })
        .await;
    }
}

#[tokio::test]
async fn utc_cron_matching_uses_utc_calendar_fields() {
    check_timezone(false, false).await;
}

#[tokio::test]
async fn local_cron_matching_uses_local_calendar_fields() {
    check_timezone(true, false).await;
}

#[tokio::test]
async fn utc_backfill_matching_uses_utc_calendar_fields() {
    check_timezone(false, true).await;
}

#[tokio::test]
async fn local_backfill_matching_uses_local_calendar_fields() {
    check_timezone(true, true).await;
}
