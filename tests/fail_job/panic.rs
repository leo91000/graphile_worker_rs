use super::*;

#[tokio::test]
async fn test_job_panics_are_recorded_as_failures() {
    #[derive(Serialize, Deserialize)]
    struct PanicJob {
        message: String,
    }

    impl TaskHandler for PanicJob {
        const IDENTIFIER: &'static str = "panic_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            if self.message.is_empty() {
                return Ok::<(), String>(());
            }

            panic!("{}", self.message);
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<PanicJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();
        utils
            .add_job(
                PanicJob {
                    message: "panic payload".to_string(),
                },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.task_identifier, "panic_job");
        assert_eq!(job.attempts, 1, "Job should have one failed attempt");
        assert!(
            job.last_error
                .as_ref()
                .is_some_and(|error| error.contains("panic payload")),
            "Error message should contain the panic payload"
        );
    })
    .await;
}
