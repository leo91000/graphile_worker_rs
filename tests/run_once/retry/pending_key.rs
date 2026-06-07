use super::*;

#[tokio::test]
async fn schedules_a_new_job_if_the_existing_is_pending_retry() {
    static JOB5_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Deserialize, Serialize)]
    struct Job5 {
        succeed: bool,
    }

    impl TaskHandler for Job5 {
        const IDENTIFIER: &'static str = "job5";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB5_CALL_COUNT.increment().await;
            if !self.succeed {
                return Err("fail".to_string());
            }

            Ok(())
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job5>()
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job that is initially set to fail
        worker
            .create_utils()
            .add_job(
                Job5 { succeed: false },
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
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
            .add_job(
                Job5 { succeed: true },
                JobSpec {
                    job_key: Some("abc".into()),
                    run_at: Some(Utc::now() - Duration::seconds(1)),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to update job");

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
