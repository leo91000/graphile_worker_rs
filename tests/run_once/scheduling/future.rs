use super::*;

#[tokio::test]
async fn it_should_supports_future_scheduled_jobs() {
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

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        run_at: Some(Utc::now() + chrono::Duration::seconds(3)),
                        ..Default::default()
                    },
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
