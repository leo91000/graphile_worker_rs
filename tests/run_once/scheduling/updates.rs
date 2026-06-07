use super::*;

#[tokio::test]
async fn it_shoud_allow_update_of_pending_jobs() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: String,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
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
                JobSpec {
                    run_at: Some(run_at),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
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
                JobSpec {
                    run_at: Some(now),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
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
async fn job_details_are_reset_if_not_specified_in_update() {
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

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
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
            .add_job(
                Job3 { a: 1 },
                JobSpec {
                    queue_name: Some("queue1".into()),
                    run_at: Some(run_at),
                    max_attempts: Some(10),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
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
            .add_job(
                Job3 { a: 1 }, // maintaining payload for comparison
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
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
            .add_job(
                Job3 { a: 2 },
                JobSpec {
                    queue_name: Some("queue2".into()),
                    run_at: Some(run_at2),
                    max_attempts: Some(100),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
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
