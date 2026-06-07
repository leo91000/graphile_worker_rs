use super::*;

#[tokio::test]
async fn test_job_fails_with_retry_delay() {
    #[derive(Serialize, Deserialize)]
    struct RetryJob {
        counter: usize,
    }

    static EXECUTION_COUNTER: OnceLock<Arc<Mutex<usize>>> = OnceLock::new();

    impl TaskHandler for RetryJob {
        const IDENTIFIER: &'static str = "retry_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            let counter = EXECUTION_COUNTER
                .get_or_init(|| Arc::new(Mutex::new(0)))
                .clone();
            let mut count = counter.lock().await;
            *count += 1;

            Err("Job failed but should retry".to_string())
        }
    }

    helpers::with_test_db(|test_db| async move {
        let _ = EXECUTION_COUNTER.get_or_init(|| Arc::new(Mutex::new(0)));

        let worker = test_db
            .create_worker_options()
            .define_job::<RetryJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();
        utils
            .add_job(
                RetryJob { counter: 0 },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        worker.run_once().await.expect("Failed to run worker");

        let count = *EXECUTION_COUNTER.get().unwrap().lock().await;
        assert_eq!(count, 1, "Job should have been executed once");

        test_db.make_jobs_run_now("retry_job").await;
        worker.run_once().await.expect("Failed to run worker");

        let count = *EXECUTION_COUNTER.get().unwrap().lock().await;
        assert_eq!(count, 2, "Job should have been executed twice");

        test_db.make_jobs_run_now("retry_job").await;
        worker.run_once().await.expect("Failed to run worker");

        let count = *EXECUTION_COUNTER.get().unwrap().lock().await;
        assert_eq!(count, 3, "Job should have been executed three times");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 3, "Job should have three failed attempts");
        assert_eq!(job.max_attempts, 3, "Max attempts should be 3");
    })
    .await;
}
