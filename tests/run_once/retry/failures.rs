use super::*;

#[tokio::test]
async fn it_should_schedule_errors_for_retry() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
            Err("fail".to_string())
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
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
                    JobSpec {
                        queue_name: Some("myqueue".to_string()),
                        ..Default::default()
                    },
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
            let now = Utc::now();
            let start_diff_ms = (job.run_at.timestamp_millis() - start.timestamp_millis()).abs();
            assert!(
                job.run_at >= start || start_diff_ms <= 50,
                "job.run_at should be >= start or within 50ms tolerance, diff: {}ms",
                start_diff_ms
            );
            assert!(job.run_at <= now, "job.run_at should be <= now");

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
            let retry_delay = chrono::Duration::milliseconds(2719);
            let clock_tolerance = chrono::Duration::milliseconds(50);
            assert!(job.run_at >= start + retry_delay - clock_tolerance);
            assert!(job.run_at <= Utc::now() + retry_delay + clock_tolerance);

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

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
            Err("fail 2".to_string())
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        queue_name: Some("myqueue".to_string()),
                        ..Default::default()
                    },
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
            let retry_delay = chrono::Duration::milliseconds(7389);
            let clock_tolerance = chrono::Duration::milliseconds(50);
            assert!(job.run_at >= start + retry_delay - clock_tolerance);
            assert!(job.run_at <= Utc::now() + retry_delay + clock_tolerance);

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
