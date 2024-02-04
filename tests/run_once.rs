use std::ops::Add;

use crate::helpers::StaticCounter;
use chrono::{Timelike, Utc};
use graphile_worker::JobSpec;
use serde_json::json;

mod helpers;

#[tokio::test]
async fn it_should_run_jobs() {
    static JOB2_CALL_COUNT: StaticCounter = StaticCounter::new();
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                JOB3_CALL_COUNT.increment().await;
                Ok(()) as Result<(), ()>
            })
            .define_raw_job("job2", |_, _: serde_json::Value| async move {
                JOB2_CALL_COUNT.increment().await;
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        let start = Utc::now();
        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    Some(JobSpec {
                        queue_name: Some("myqueue".to_string()),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");
        }

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert!(job.run_at > start);
        assert!(job.run_at < Utc::now());
        let job_queues = test_db.get_job_queues().await;
        assert_eq!(job_queues.len(), 1);
        let job_queue = &job_queues[0];
        assert_eq!(job_queue.queue_name, "myqueue");
        assert_eq!(job_queue.job_count, 1);
        assert_eq!(job_queue.locked_at, None);
        assert_eq!(job_queue.locked_by, None);

        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);
        assert_eq!(JOB2_CALL_COUNT.get().await, 0);
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);
    })
    .await;
}

#[tokio::test]
async fn it_should_schedule_errors_for_retry() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                JOB3_CALL_COUNT.increment().await;
                Err("fail".to_string()) as Result<(), String>
            })
            .init()
            .await
            .expect("Failed to create worker");

        let start = Utc::now();
        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    Some(JobSpec {
                        queue_name: Some("myqueue".to_string()),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");
        }

        {
            let jobs = test_db.get_jobs().await;
            assert_eq!(jobs.len(), 1);
            let job = &jobs[0];
            assert_eq!(job.task_identifier, "job3");
            assert_eq!(job.payload, json!({ "a": 1 }));
            assert!(job.run_at > start);
            assert!(job.run_at < Utc::now());

            let job_queues = test_db.get_job_queues().await;
            assert_eq!(job_queues.len(), 1);
            let job_queue = &job_queues[0];
            assert_eq!(job_queue.queue_name, "myqueue");
            assert_eq!(job_queue.job_count, 1);
            assert_eq!(job_queue.locked_at, None);
            assert_eq!(job_queue.locked_by, None);
        }

        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        {
            let jobs = test_db.get_jobs().await;
            assert_eq!(jobs.len(), 1);
            let job = &jobs[0];
            assert_eq!(job.task_identifier, "job3");
            assert_eq!(job.attempts, 1);
            assert_eq!(job.max_attempts, 25);
            assert_eq!(
                job.last_error,
                Some("TaskError(\"\\\"fail\\\"\")".to_string())
            );
            // It's the first attempt, so delay is exp(1) ~= 2.719 seconds
            assert!(job.run_at > start + chrono::Duration::milliseconds(2719));
            assert!(job.run_at < Utc::now() + chrono::Duration::milliseconds(2719));

            let job_queues = test_db.get_job_queues().await;
            assert_eq!(job_queues.len(), 1);
            let q = &job_queues[0];
            assert_eq!(q.queue_name, "myqueue");
            assert_eq!(q.job_count, 1);
            assert_eq!(q.locked_at, None);
            assert_eq!(q.locked_by, None);
        }
    })
    .await;
}

#[tokio::test]
async fn it_should_retry_jobs() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                JOB3_CALL_COUNT.increment().await;
                Err("fail 2".to_string()) as Result<(), String>
            })
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    Some(JobSpec {
                        queue_name: Some("myqueue".to_string()),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");
        }

        // Run the job (it will error)
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // Should do nothing the second time, because it's queued for the future (assuming we run this fast enough afterwards!)
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // Tell the job to run now
        test_db.make_jobs_run_now("job3").await;
        let start = Utc::now();
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 2);

        // Should be rejected again
        {
            let jobs = test_db.get_jobs().await;
            assert_eq!(jobs.len(), 1);
            let job = &jobs[0];
            assert_eq!(job.task_identifier, "job3");
            assert_eq!(job.attempts, 2);
            assert_eq!(job.max_attempts, 25);
            assert_eq!(
                job.last_error,
                Some("TaskError(\"\\\"fail 2\\\"\")".to_string())
            );
            // It's the second attempt, so delay is exp(2) ~= 7.389 seconds
            assert!(job.run_at > start + chrono::Duration::milliseconds(7389));
            assert!(job.run_at < Utc::now() + chrono::Duration::milliseconds(7389));

            let job_queues = test_db.get_job_queues().await;
            assert_eq!(job_queues.len(), 1);
            let q = &job_queues[0];
            assert_eq!(q.queue_name, "myqueue");
            assert_eq!(q.job_count, 1);
            assert_eq!(q.locked_at, None);
            assert_eq!(q.locked_by, None);
        }
    })
    .await;
}

#[tokio::test]
async fn it_should_supports_future_scheduled_jobs() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                JOB3_CALL_COUNT.increment().await;
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    Some(JobSpec {
                        run_at: Some(Utc::now() + chrono::Duration::seconds(3)),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");
        }

        // Run all the jobs now (none should run)
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 0);

        // Still not ready
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 0);

        // Make the job ready
        test_db.make_jobs_run_now("job3").await;
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // It should be successful
        {
            let jobs = test_db.get_jobs().await;
            assert_eq!(jobs.len(), 0);
            let job_queues = test_db.get_job_queues().await;
            assert_eq!(job_queues.len(), 0);
        }
    })
    .await;
}

#[tokio::test]
async fn it_shoud_allow_update_of_pending_jobs() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, payload: serde_json::Value| async move {
                JOB3_CALL_COUNT.increment().await;
                assert_eq!(payload, json!({ "a": "right" }));
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        // Rust is more precise than postgres, so we need to remove the nanoseconds
        let run_at = Utc::now()
            .add(chrono::Duration::seconds(60))
            .with_nanosecond(0)
            .unwrap();
        let utils = worker.create_utils();
        // Schedule a future job - note incorrect payload
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "wrong" }),
                Some(JobSpec {
                    run_at: Some(run_at),
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Assert that it has an entry in jobs / job_queues
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!(job.run_at, run_at);

        // Run all jobs (none are ready)
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 0);

        // update the job to run immediately with correct payload
        let now = Utc::now().with_nanosecond(0).unwrap();
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "right" }),
                Some(JobSpec {
                    run_at: Some(now),
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Assert that it has updated the existing entry and not created a new one
        let updated_jobs = test_db.get_jobs().await;
        assert_eq!(updated_jobs.len(), 1);
        let updated_job = &updated_jobs[0];
        assert_eq!(job.id, updated_job.id);
        assert_eq!(updated_job.run_at, now);

        // Run the task
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);
    })
    .await;
}

#[tokio::test]
async fn it_schedules_a_new_job_if_existing_is_completed() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                JOB3_CALL_COUNT.increment().await;
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();
        // Schedule a job to run immediately with job key "abc" and a payload.
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "first" }),
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Run the worker once to process the job.
        worker.run_once().await.expect("Failed to run worker");

        // Assert the job has been called once.
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // Attempt to update the job. Since the previous job is completed, it should schedule a new one.
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "second" }),
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Run the worker again to process the new job.
        worker.run_once().await.expect("Failed to run worker");

        // Assert the job has been called twice, indicating the new job was scheduled and run.
        assert_eq!(JOB3_CALL_COUNT.get().await, 2);
    })
    .await;
}
