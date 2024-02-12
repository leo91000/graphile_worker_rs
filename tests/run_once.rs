use std::{collections::VecDeque, ops::Add, rc::Rc};

use crate::helpers::StaticCounter;
use chrono::{Duration, Timelike, Utc};
use graphile_worker::{JobSpec, WorkerContext};
use serde_json::json;
use tokio::{
    sync::{oneshot, Mutex, OnceCell},
    task::spawn_local,
    time::{sleep, Instant},
};

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

#[tokio::test]
async fn schedules_a_new_job_if_existing_is_being_processed() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RX1: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();
    static RX2: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();

    helpers::with_test_db(|test_db| async move {
        let (tx1, rx1) = oneshot::channel::<()>();
        let (tx2, rx2) = oneshot::channel::<()>();
        RX1.set(Mutex::new(Some(rx1))).unwrap();
        RX2.set(Mutex::new(Some(rx2))).unwrap();

        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", move |_, _: serde_json::Value| async move {
                let n = JOB3_CALL_COUNT.increment().await;
                match n {
                    1 => {
                        let mut rx_opt = RX1.get().unwrap().lock().await;
                        if let Some(rx) = rx_opt.take() {
                            rx.await.unwrap();
                        }
                    }
                    2 => {
                        let mut rx_opt = RX2.get().unwrap().lock().await;
                        if let Some(rx) = rx_opt.take() {
                            rx.await.unwrap();
                        }
                    }
                    _ => unreachable!("Job3 should only be called twice"),
                };
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule the first job
        worker
            .create_utils()
            .add_raw_job(
                "job3",
                json!({ "a": "first" }),
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add first job");

        let worker = Rc::new(worker);

        tracing::info!("Starting worker");
        // Run the first job
        let run_once_1 = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run_once().await.expect("Failed to run worker");
            }
        });

        tracing::info!("Waiting for first job to be picked up");
        // Wait for the first job to be picked up
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job3 should have been executed by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // Schedule a new job with the same key while the first one is being processed
        worker
            .create_utils()
            .add_raw_job(
                "job3",
                json!({ "a": "second" }),
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add second job");

        // Complete the first job
        tx1.send(()).unwrap();

        run_once_1.await.expect("Failed to run worker");

        // Run the worker again to pick up the second job
        let run_once_2 = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run_once().await.expect("Failed to run worker");
            }
        });

        // Complete the second job
        tx2.send(()).unwrap();

        run_once_2.await.expect("Failed to run worker");

        // Ensure both jobs have been processed
        assert_eq!(JOB3_CALL_COUNT.get().await, 2);
    })
    .await;
}

#[tokio::test]
async fn schedules_a_new_job_if_the_existing_is_pending_retry() {
    static JOB5_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job5Args {
        succeed: bool,
    }

    #[graphile_worker::task]
    async fn job5(args: Job5Args, _: WorkerContext) -> Result<(), String> {
        JOB5_CALL_COUNT.increment().await;
        if !args.succeed {
            return Err("fail".to_string());
        }

        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job(job5)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job that is initially set to fail
        worker
            .create_utils()
            .add_job::<job5>(
                Job5Args { succeed: false },
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Run the worker to process the job, which should fail
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(
            JOB5_CALL_COUNT.get().await,
            1,
            "job5 should have been called once"
        );

        // Check that the job is scheduled for retry
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "There should be one job scheduled for retry");
        let job = &jobs[0];
        assert_eq!(job.attempts, 1, "The job should have one failed attempt");
        let last_error = job
            .last_error
            .as_ref()
            .expect("The job should have a last error");
        assert!(
            last_error.contains("fail"),
            "The job's last error should contain 'fail'"
        );

        // No job should run now as it is scheduled for the future
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(
            JOB5_CALL_COUNT.get().await,
            1,
            "job5 should still be called only once"
        );

        // Update the job to succeed
        worker
            .create_utils()
            .add_job::<job5>(
                Job5Args { succeed: true },
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    run_at: Some(Utc::now()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to update job");
        //
        // Assert that the job was updated and not created as a new one
        let updated_jobs = test_db.get_jobs().await;
        assert_eq!(
            updated_jobs.len(),
            1,
            "There should still be only one job in the database"
        );
        let updated_job = &updated_jobs[0];
        assert_eq!(
            updated_job.attempts, 0,
            "The job's attempts should be reset to 0"
        );
        assert!(
            updated_job.last_error.is_none(),
            "The job's last error should be cleared"
        );

        // Run the worker again, and the job should now succeed
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(
            JOB5_CALL_COUNT.get().await,
            2,
            "job5 should have been called twice"
        );
    })
    .await;
}

#[tokio::test]
async fn job_details_are_reset_if_not_specified_in_update() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: i32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        JOB3_CALL_COUNT.increment().await;

        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        let run_at = Utc::now()
            .add(Duration::seconds(3))
            .with_nanosecond(0)
            .unwrap();

        // Schedule a future job
        worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 1 },
                Some(JobSpec {
                    queue_name: Some("queue1".into()),
                    run_at: Some(run_at),
                    max_attempts: Some(10),
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Assert initial job details
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let original = &jobs[0];
        assert_eq!(original.attempts, 0);
        assert_eq!(original.key, Some("abc".to_string()));
        assert_eq!(original.max_attempts, 10);
        assert_eq!(original.payload, serde_json::json!({"a": 1}));
        assert_eq!(original.queue_name, Some("queue1".to_string()));
        assert_eq!(original.run_at, run_at);
        assert_eq!(original.task_identifier, "job3");

        // Update job without specifying new details
        worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 1 }, // maintaining payload for comparison
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to update job");

        // Check that omitted details have reverted to default values
        let updated_jobs = test_db.get_jobs().await;
        assert_eq!(updated_jobs.len(), 1);
        let updated_job = &updated_jobs[0];
        assert_eq!(updated_job.attempts, 0);
        assert_eq!(updated_job.key, Some("abc".to_string()));
        assert_eq!(updated_job.max_attempts, 25); // Default value for max_attempts
        assert_eq!(updated_job.payload, serde_json::json!({"a": 1})); // Payload remains unchanged
        assert_eq!(updated_job.queue_name, None); // Queue name should not change unless explicitly updated
        assert_ne!(updated_job.run_at, run_at); // `run_at` should revert to the default (current time)

        // Update job with new details
        let run_at2 = Utc::now()
            .add(Duration::seconds(5))
            .with_nanosecond(0)
            .unwrap();

        worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 2 },
                Some(JobSpec {
                    queue_name: Some("queue2".into()),
                    run_at: Some(run_at2),
                    max_attempts: Some(100),
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to update job with new details");

        // Check that details have changed
        let final_jobs = test_db.get_jobs().await;
        assert_eq!(final_jobs.len(), 1);
        let final_job = &final_jobs[0];
        assert_eq!(final_job.attempts, 0);
        assert_eq!(final_job.key, Some("abc".to_string()));
        assert_eq!(final_job.max_attempts, 100);
        assert_eq!(final_job.payload, serde_json::json!({"a": 2}));
        assert_eq!(final_job.queue_name, Some("queue2".to_string()));
        assert_eq!(final_job.run_at, run_at2);
        assert_eq!(final_job.task_identifier, "job3"); // Assuming the task identifier remains unchanged
    })
    .await;
}

#[tokio::test]
async fn pending_jobs_can_be_removed() {
    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: String,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        // Job logic here
        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job
        worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: "1".into() },
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Assert that it has an entry in jobs
        let jobs_before_removal = test_db.get_jobs().await;
        assert_eq!(
            jobs_before_removal.len(),
            1,
            "There should be one job scheduled"
        );

        // Remove the job
        worker
            .create_utils()
            .remove_job("abc")
            .await
            .expect("Failed to remove job");

        // Check there are no jobs
        let jobs_after_removal = test_db.get_jobs().await;
        assert_eq!(
            jobs_after_removal.len(),
            0,
            "There should be no jobs scheduled after removal"
        );
    })
    .await;
}

#[tokio::test]
async fn jobs_in_progress_cannot_be_removed() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RX: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: i32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        JOB3_CALL_COUNT.increment().await;
        // Wait on the receiver, simulating a deferred operation
        let mut rx_mutex_guard = RX.get().unwrap().lock().await;
        if let Some(rx) = rx_mutex_guard.take() {
            rx.await.unwrap();
        }
        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        RX.set(Mutex::new(Some(rx))).unwrap();

        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job
        let utils = worker.create_utils();
        utils
            .add_job::<job3>(
                Job3Args { a: 123 },
                Some(JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        // Check it was inserted
        let jobs_before = test_db.get_jobs().await;
        assert_eq!(jobs_before.len(), 1, "Job should be scheduled");

        // Run the worker in a separate task to pick up the job
        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        // Wait for the job to be picked up
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job3 should have been picked up by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(JOB3_CALL_COUNT.get().await, 1, "Job should be in progress");

        // Attempt to remove the job
        utils
            .remove_job("abc")
            .await
            .expect("Failed to attempt job removal");

        // Check it was not removed
        let jobs_during = test_db.get_jobs().await;
        assert_eq!(
            jobs_during.len(),
            1,
            "Job should not be removed while in progress"
        );

        // Complete the job
        tx.send(()).expect("Failed to send completion signal");

        // Wait for the worker task to complete
        worker_handle.await.expect("Worker task failed");

        // Verify the job completed
        assert_eq!(JOB3_CALL_COUNT.get().await, 1, "Job should have completed");

        // Check the job is removed after completion
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "Job should be removed after completion"
        );
    })
    .await;
}

#[tokio::test]
async fn runs_jobs_asynchronously() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static JOB_RX: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: i32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        JOB3_CALL_COUNT.increment().await;
        let mut rx = JOB_RX.get().unwrap().lock().await;
        if let Some(receiver) = rx.take() {
            receiver.await.unwrap();
        }
        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let (tx, rx) = oneshot::channel::<()>();
        JOB_RX.set(Mutex::new(Some(rx))).unwrap();

        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job
        let start = Utc::now();
        worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 1 },
                Some(JobSpec {
                    queue_name: Some("myqueue".into()),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");

        let worker_id = worker.worker_id().to_owned();

        // Start the worker to pick up the job
        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        // Wait for the job to be picked up but not completed
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job3 should have been picked up by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            1,
            "Job should be in progress but not completed"
        );

        // Check job and queue state to ensure they reflect an in-progress job
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "There should be one job in progress");
        let job = &jobs[0];
        assert_eq!(job.task_identifier, "job3");
        assert_eq!(job.payload, serde_json::json!({ "a": 1 }));
        assert!(
            job.run_at >= start && job.run_at <= Utc::now(),
            "Job run_at should be within expected range"
        );
        assert_eq!(job.attempts, 1, "Job attempts should be incremented");
        let job_queues = test_db.get_job_queues().await;
        assert_eq!(
            job_queues.len(),
            1,
            "There should be one queue with a job in progress"
        );
        let q = &job_queues[0];
        assert_eq!(&q.queue_name, job.queue_name.as_ref().unwrap());
        assert_eq!(q.job_count, 1);
        assert!(q.locked_at.is_some(), "The job should be locked");
        assert!(
            q.locked_at.unwrap() >= start && q.locked_at.unwrap() <= Utc::now(),
            "The lock time should be within expected range"
        );
        assert_eq!(q.locked_by, Some(worker_id));

        // Complete the job
        tx.send(()).expect("Failed to send completion signal");
        worker_handle.await.expect("Worker task failed");

        // Verify the job completed
        assert_eq!(JOB3_CALL_COUNT.get().await, 1, "Job should have completed");

        // Check the job is removed after completion
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "Job should be removed after completion"
        );
    })
    .await;
}

#[tokio::test]
async fn runs_jobs_in_parallel() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RXS: OnceCell<Mutex<Vec<oneshot::Receiver<()>>>> = OnceCell::const_new();
    RXS.set(Mutex::new(vec![])).unwrap();
    let mut txs = vec![];

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: i32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        let rx = RXS
            .get()
            .expect("OnceCell should be set globally at the beginning of the test")
            .lock()
            .await
            .remove(0); // Obtain the receiver for this job
        JOB3_CALL_COUNT.increment().await;
        rx.await
            .expect("The receiver should not be dropped before the job completes");
        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .concurrency(10)
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");
        let worker = Rc::new(worker);

        // Schedule 5 jobs in different queues
        for i in 1..=5 {
            let (tx, rx) = oneshot::channel::<()>();
            txs.push(tx);
            RXS.get().unwrap().lock().await.push(rx);

            worker
                .create_utils()
                .add_job::<job3>(
                    Job3Args { a: i },
                    Some(JobSpec {
                        queue_name: Some(format!("queue_{}", i)),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");
        }

        let start = Utc::now();

        // Run 5 worker instances in parallel
        let mut handles = vec![];
        let worker_clone = worker.clone();
        handles.push(spawn_local(async move {
            worker_clone.run_once().await.expect("Failed to run worker");
        }));

        // Wait for all jobs are picked up
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 20 {
                panic!("Job3 should have been picked up by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should be in progress"
        );

        // Verify jobs and queues state
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 5, "There should be 5 jobs in progress");

        let job_queues = test_db.get_job_queues().await;
        assert_eq!(job_queues.len(), 5, "There should be 5 job queues");
        for q in job_queues {
            assert_eq!(q.job_count, 1, "Each queue should have one job");
            assert!(q.locked_at.unwrap() >= start && q.locked_at.unwrap() <= Utc::now());
        }

        // Complete all jobs
        for tx in txs {
            tx.send(()).expect("Failed to send completion signal");
        }

        // Wait for all worker instances to finish
        for handle in handles {
            handle.await.expect("Worker task failed");
        }

        // Verify all jobs completed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should have completed"
        );

        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "All jobs should be removed after completion"
        );
    })
    .await;
}

#[tokio::test]
async fn single_worker_runs_jobs_in_series_purges_all_before_exit() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RXS: OnceCell<Mutex<VecDeque<oneshot::Receiver<()>>>> = OnceCell::const_new();
    RXS.set(Mutex::new(VecDeque::new())).unwrap();
    let mut txs = vec![];

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: i32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        let rx = RXS.get().unwrap().lock().await.pop_front().unwrap(); // Obtain the receiver for the current job
        rx.await.unwrap(); // Wait for the signal to complete the job
        JOB3_CALL_COUNT.increment().await;
        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule 5 jobs
        for _ in 1..=5 {
            let (tx, rx) = oneshot::channel::<()>();
            txs.push(tx);
            RXS.get().unwrap().lock().await.push_back(rx);

            worker
                .create_utils()
                .add_job::<job3>(Job3Args { a: 1 }, None)
                .await
                .expect("Failed to add job");
        }

        // Start the worker to pick up and run the jobs
        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        // Sequentially complete each job and verify progress
        let mut i = 0;
        for tx in txs {
            i += 1;
            // Complete the current job
            tx.send(()).expect("Failed to send completion signal");

            // Wait a brief moment to ensure the job is picked up
            sleep(tokio::time::Duration::from_millis(100)).await;
            assert_eq!(
                JOB3_CALL_COUNT.get().await,
                i,
                "Job {} should be in progress",
                i,
            );
        }

        // Wait for the worker to finish processing all jobs
        worker_handle.await.expect("Worker task failed");

        // Verify all jobs were completed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should have completed"
        );

        // Check that all jobs are purged from the database
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "All jobs should be removed after completion"
        );
    })
    .await;
}

#[tokio::test]
async fn jobs_added_to_the_same_queue_will_be_ran_serially_even_if_multiple_workers() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RXS: OnceCell<Mutex<VecDeque<oneshot::Receiver<()>>>> = OnceCell::const_new();
    RXS.set(Mutex::new(VecDeque::new())).unwrap();
    let mut txs = vec![];

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: i32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
        let rx = RXS.get().unwrap().lock().await.pop_front().unwrap(); // Obtain the receiver for the current job
        rx.await.unwrap(); // Wait for the signal to complete the job
        JOB3_CALL_COUNT.increment().await;
        Ok(())
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");
        let worker = Rc::new(worker);

        // Schedule 5 jobs to the same queue
        for _ in 1..=5 {
            let (tx, rx) = oneshot::channel::<()>();
            txs.push(tx);
            RXS.get().unwrap().lock().await.push_back(rx);

            worker
                .create_utils()
                .add_job::<job3>(
                    Job3Args { a: 1 },
                    Some(JobSpec {
                        queue_name: Some("serial".into()),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");
        }

        // Start multiple worker instances to process the jobs
        let mut handles = vec![];
        for _ in 0..3 {
            let worker = worker.clone();
            handles.push(spawn_local(async move {
                worker.run_once().await.expect("Failed to run worker");
            }));
        }

        // Sequentially complete each job and verify progress
        for i in 1..=5 {
            // Give other workers a chance to interfere
            sleep(tokio::time::Duration::from_millis(50)).await;

            // Complete the current job
            txs.remove(0)
                .send(())
                .expect("Failed to send completion signal");

            // Wait for the job to be picked up
            let start_time = Instant::now();
            while JOB3_CALL_COUNT.get().await < i {
                if start_time.elapsed().as_secs() > 5 {
                    panic!("Job3 should have been picked up by now");
                }
                sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // Wait for all worker instances to finish
        for handle in handles {
            handle.await.expect("Worker task failed");
        }

        // Verify all jobs were completed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should have completed"
        );

        // Check that all jobs are purged from the database
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "All jobs should be removed after completion"
        );
    })
    .await;
}
