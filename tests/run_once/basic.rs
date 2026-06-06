use super::*;

#[tokio::test]
async fn it_should_run_jobs() {
    static JOB2_CALL_COUNT: StaticCounter = StaticCounter::new();
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job2 {
        a: u32,
    }

    impl TaskHandler for Job2 {
        const IDENTIFIER: &'static str = "job2";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB2_CALL_COUNT.increment().await;
        }
    }

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
            .define_job::<Job2>()
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

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];

        let start_diff_ms = (job.run_at.timestamp_millis() - start.timestamp_millis()).abs();
        assert!(
            job.run_at >= start || start_diff_ms <= 50,
            "job.run_at should be >= start or within 50ms tolerance, diff: {}ms",
            start_diff_ms
        );
        assert!(job.run_at <= Utc::now(), "job.run_at should be <= now");
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
