use super::*;

#[tokio::test]
async fn test_job_permanent_failure() {
    #[derive(Serialize, Deserialize)]
    struct PermanentFailureJob {
        should_fail: bool,
    }

    impl TaskHandler for PermanentFailureJob {
        const IDENTIFIER: &'static str = "permanent_failure_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            if self.should_fail {
                Err("This job failed".to_string())
            } else {
                Ok(())
            }
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<PermanentFailureJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();
        utils
            .add_job(
                PermanentFailureJob { should_fail: true },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 1, "Job should have one failed attempt");

        sqlx::query(
            r#"
            UPDATE graphile_worker._private_jobs
            SET attempts = max_attempts,
                last_error = 'Manually marked as permanently failed'
            WHERE task_id = (
                SELECT id FROM graphile_worker._private_tasks
                WHERE identifier = 'permanent_failure_job'
            )
            "#,
        )
        .execute(&test_db.test_pool)
        .await
        .expect("Failed to update job");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(
            job.attempts, job.max_attempts,
            "Job should be marked as max attempts"
        );
        assert!(
            job.last_error
                .as_ref()
                .unwrap()
                .contains("Manually marked as permanently failed"),
            "Error message should be updated"
        );

        worker.run_once().await.expect("Failed to run worker");

        let jobs_after = test_db.get_jobs().await;
        assert_eq!(jobs_after.len(), 1);
        assert_eq!(jobs_after[0].attempts, job.max_attempts);
    })
    .await;
}
