use chrono::{Duration, DurationRound, Local};
use graphile_worker::IntoTaskHandlerResult;
use graphile_worker::Worker;
use serde::{Deserialize, Serialize};
use sqlx::query;
use tokio::{task::spawn_local, time::Instant};

use crate::helpers::{with_test_db, StaticCounter};
use graphile_worker::{TaskHandler, WorkerContext};

mod helpers;

#[tokio::test]
async fn register_identifiers() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let test_pool = test_db.test_pool.clone();
        let worker_handle = spawn_local(async move {
            Worker::options()
                .pg_pool(test_pool)
                .concurrency(3)
                .define_job::<Job3>()
                .with_crontab(
                    r#"
                        0 */4 * * * do_it ?fill=1d
                    "#,
                )
                .expect("Invalid crontab")
                .init()
                .await
                .expect("Failed to create worker")
                .run()
                .await
                .expect("Failed to run worker");
        });

        let mut known_crontabs = test_db.get_known_crontabs().await;
        let start = Instant::now();
        // Wait for known crontabs to be registered
        while known_crontabs.is_empty() {
            if start.elapsed().as_secs() > 5 {
                panic!("Crontab should have been registered by now");
            }
            known_crontabs = test_db.get_known_crontabs().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let known_crontab = &known_crontabs[0];
        assert_eq!(known_crontab.identifier(), "do_it");
        assert!(known_crontab.known_since() < &chrono::Utc::now());
        assert!(known_crontab.last_execution().is_none());

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);

        worker_handle.abort();
    })
    .await;
}

#[tokio::test]
async fn backfills_if_identifier_already_registered_5h_ago() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let now = Local::now();
        let four_hours = Duration::hours(4);
        let expected_time = now
            .duration_trunc(four_hours)
            .expect("Failed to truncate time");

        query(
            r#"
            insert into graphile_worker._private_known_crontabs as known_crontabs (
                identifier,
                known_since,
                last_execution
            )
            values (
                'do_it',
                NOW() - interval '14 days',
                NOW() - interval '5 hours'
            )
        "#,
        )
        .execute(&test_db.test_pool)
        .await
        .expect("Failed to insert known crontab");

        let test_pool = test_db.test_pool.clone();
        let worker_handle = spawn_local(async move {
            Worker::options()
                .pg_pool(test_pool)
                .concurrency(3)
                .with_crontab(
                    r#"
                        0 */4 * * * do_it ?fill=1d
                    "#,
                )
                .expect("Invalid crontab")
                .init()
                .await
                .expect("Failed to create worker")
                .run()
                .await
                .expect("Failed to run worker");
        });

        // We wait for the assertion to be true, or we panic after 5 seconds
        // This is to give time to the worker to schedule the backfill jobs
        let start_time = Instant::now();
        loop {
            let known_crontabs = test_db.get_known_crontabs().await;
            assert_eq!(known_crontabs.len(), 1);
            let known_crontab = &known_crontabs[0];
            assert_eq!(known_crontab.identifier(), "do_it");
            assert!(known_crontab.known_since() < &chrono::Utc::now());
            assert!(known_crontab.last_execution().is_some());

            let last_execution = known_crontab.last_execution().unwrap();
            let is_expected_time = last_execution == expected_time;
            let is_expected_time_plus_4h = last_execution == (expected_time + four_hours);

            if start_time.elapsed().as_secs() > 5 || is_expected_time || is_expected_time_plus_4h {
                assert!(
                    is_expected_time || is_expected_time_plus_4h,
                    r#"
                        There's a small window every 4 hours where the expect might fail
                        due to the clock advancing, so we account for that by checking 
                        both of the expected times.

                        Last execution: {:?}
                        Expected time: {:?}
                        Expected time + 4h: {:?}
                    "#,
                    last_execution,
                    expected_time,
                    expected_time + four_hours,
                );
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let jobs = test_db.get_jobs().await;
        assert!(
            jobs.len() == 1 || jobs.len() == 2,
            "It's a 5 hour window for a job that runs every 4 hours, there should be 1 or 2 jobs"
        );
        assert!(jobs.iter().all(|job| &job.task_identifier == "do_it"));

        worker_handle.abort();
    })
    .await;
}

#[tokio::test]
async fn backfills_if_identifier_already_registered_25h_ago() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let now = Local::now();
        let four_hours = Duration::hours(4);
        let expected_time = now
            .duration_trunc(four_hours)
            .expect("Failed to truncate time");

        query(
            r#"
            insert into graphile_worker._private_known_crontabs as known_crontabs (
                identifier,
                known_since,
                last_execution
            )
            values (
                'do_it',
                NOW() - interval '14 days',
                NOW() - interval '25 hours'
            )
        "#,
        )
        .execute(&test_db.test_pool)
        .await
        .expect("Failed to insert known crontab");

        let test_pool = test_db.test_pool.clone();
        let worker_handle = spawn_local(async move {
            Worker::options()
                .pg_pool(test_pool)
                .concurrency(3)
                .with_crontab(
                    r#"
                        0 */4 * * * do_it ?fill=1d
                    "#,
                )
                .expect("Invalid crontab")
                .init()
                .await
                .expect("Failed to create worker")
                .run()
                .await
                .expect("Failed to run worker");
        });

        // We wait for the assertion to be true, or we panic after 5 seconds
        // This is to give time to the worker to schedule the backfill jobs
        let start_time = Instant::now();
        loop {
            let known_crontabs = test_db.get_known_crontabs().await;
            assert_eq!(known_crontabs.len(), 1);
            let known_crontab = &known_crontabs[0];
            assert_eq!(known_crontab.identifier(), "do_it");
            assert!(known_crontab.known_since() < &chrono::Utc::now());
            assert!(known_crontab.last_execution().is_some());

            let last_execution = known_crontab.last_execution().unwrap();
            let is_expected_time = last_execution == expected_time;
            let is_expected_time_plus_4h = last_execution == (expected_time + four_hours);

            if start_time.elapsed().as_secs() > 5 || is_expected_time || is_expected_time_plus_4h {
                assert!(
                    is_expected_time || is_expected_time_plus_4h,
                    r#"
                        There's a small window every 4 hours where the expect might fail
                        due to the clock advancing, so we account for that by checking 
                        both of the expected times.

                        Last execution: {:?}
                        Expected time: {:?}
                        Expected time + 4h: {:?}
                    "#,
                    last_execution,
                    expected_time,
                    expected_time + four_hours,
                );
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let jobs = test_db.get_jobs().await;
        assert!(
            jobs.len() == 6 || jobs.len() == 7,
            "It's a 25 hour window for a job that runs every 4 hours, there should be 6 or 7 jobs"
        );
        assert!(jobs.iter().all(|job| &job.task_identifier == "do_it"));

        worker_handle.abort();
    })
    .await;
}
