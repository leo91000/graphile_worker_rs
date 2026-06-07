use super::*;

#[tokio::test]
async fn test_job_fails_with_error() {
    #[derive(Serialize, Deserialize)]
    struct FailingJob {
        message: String,
    }

    impl TaskHandler for FailingJob {
        const IDENTIFIER: &'static str = "failing_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            Err(format!("Failed with message: {}", self.message))
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<FailingJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();
        utils
            .add_job(
                FailingJob {
                    message: "test error".to_string(),
                },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.task_identifier, "failing_job");
        assert_eq!(job.attempts, 1, "Job should have one failed attempt");
        assert!(job.last_error.is_some(), "Job should have an error message");
        assert!(
            job.last_error
                .as_ref()
                .unwrap()
                .contains("Failed with message: test error"),
            "Error message should contain the original error"
        );

        test_db.make_jobs_run_now("failing_job").await;
        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 2, "Job should have two failed attempts");

        test_db.make_jobs_run_now("failing_job").await;
        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 3, "Job should have three failed attempts");
        assert_eq!(job.max_attempts, 3, "Max attempts should be 3");
    })
    .await;
}
