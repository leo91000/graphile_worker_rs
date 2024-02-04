use chrono::Utc;
use graphile_worker::JobSpec;
use serde_json::json;
use std::sync::OnceLock;
use tokio::sync::Mutex;

mod helpers;

#[tokio::test]
async fn it_should_run_jobs() {
    static JOB2_CALL_COUNT: OnceLock<Mutex<u32>> = OnceLock::new();
    static JOB3_CALL_COUNT: OnceLock<Mutex<u32>> = OnceLock::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                let mut job_count = JOB3_CALL_COUNT.get_or_init(|| Mutex::new(0)).lock().await;
                *job_count += 1;
                Ok(()) as Result<(), ()>
            })
            .define_raw_job("job2", |_, _: serde_json::Value| async move {
                let mut job_count = JOB2_CALL_COUNT.get_or_init(|| Mutex::new(0)).lock().await;
                *job_count += 1;
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
        assert_eq!(
            *JOB3_CALL_COUNT.get_or_init(|| Mutex::new(0)).lock().await,
            1
        );
        assert_eq!(
            *JOB2_CALL_COUNT.get_or_init(|| Mutex::new(0)).lock().await,
            0
        );
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);
    })
    .await;
}

#[tokio::test]
async fn it_should_schedule_errors_for_retry() {
    static JOB3_CALL_COUNT: OnceLock<Mutex<u32>> = OnceLock::new();

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3", |_, _: serde_json::Value| async move {
                let mut job_count = JOB3_CALL_COUNT.get_or_init(|| Mutex::new(0)).lock().await;
                *job_count += 1;
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
        assert_eq!(
            *JOB3_CALL_COUNT.get_or_init(|| Mutex::new(0)).lock().await,
            1
        );

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
