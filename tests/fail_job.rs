#![allow(dead_code)]

use graphile_worker::{IntoTaskHandlerResult, JobSpecBuilder, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;

mod helpers;

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

        // Add a job that will fail
        utils
            .add_job(
                FailingJob {
                    message: "test error".to_string(),
                },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        // Run the worker once
        worker.run_once().await.expect("Failed to run worker");

        // Check that the job is in the database with the error
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

        // Make the job run now (ignoring backoff)
        test_db.make_jobs_run_now("failing_job").await;

        // Run the worker again to retry
        worker.run_once().await.expect("Failed to run worker");

        // Check that the job is still there with two attempts
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 2, "Job should have two failed attempts");

        // Make the job run now (ignoring backoff)
        test_db.make_jobs_run_now("failing_job").await;

        // Run one more time to reach max attempts
        worker.run_once().await.expect("Failed to run worker");

        // The job should still be there but marked as permanently failed
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 3, "Job should have three failed attempts");
        assert_eq!(job.max_attempts, 3, "Max attempts should be 3");
    })
    .await;
}

#[tokio::test]
async fn test_job_fails_with_retry_delay() {
    #[derive(Serialize, Deserialize)]
    struct RetryJob {
        counter: usize,
    }

    // Create a shared counter to track number of executions
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
        // Initialize the counter
        let _ = EXECUTION_COUNTER.get_or_init(|| Arc::new(Mutex::new(0)));

        let worker = test_db
            .create_worker_options()
            .define_job::<RetryJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();

        // Add a job that will fail but with a small retry delay
        utils
            .add_job(
                RetryJob { counter: 0 },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        // Run the worker
        worker.run_once().await.expect("Failed to run worker");

        // Check the job has been executed once
        let count = *EXECUTION_COUNTER.get().unwrap().lock().await;
        assert_eq!(count, 1, "Job should have been executed once");

        // Job should have a backoff delay, but we'll make it run now
        test_db.make_jobs_run_now("retry_job").await;

        // Run the worker again
        worker.run_once().await.expect("Failed to run worker");

        // Check the job has been executed twice
        let count = *EXECUTION_COUNTER.get().unwrap().lock().await;
        assert_eq!(count, 2, "Job should have been executed twice");

        // Job should have a longer backoff delay
        test_db.make_jobs_run_now("retry_job").await;

        // Run the worker one more time
        worker.run_once().await.expect("Failed to run worker");

        // Check the job has been executed three times
        let count = *EXECUTION_COUNTER.get().unwrap().lock().await;
        assert_eq!(count, 3, "Job should have been executed three times");

        // The job should be permanently failed now
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 3, "Job should have three failed attempts");
        assert_eq!(job.max_attempts, 3, "Max attempts should be 3");
    })
    .await;
}

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

        // Add a job that will fail
        utils
            .add_job(
                PermanentFailureJob { should_fail: true },
                JobSpecBuilder::new().max_attempts(3).build(),
            )
            .await
            .expect("Failed to add job");

        // Run the worker once
        worker.run_once().await.expect("Failed to run worker");

        // Verify the job failed once
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "Job should still be in the database");
        let job = &jobs[0];
        assert_eq!(job.attempts, 1, "Job should have one failed attempt");

        // Manually fail the job using SQL to simulate a permanent failure
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

        // Verify the job is now marked as permanently failed
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

        // Try to run the worker again, but the job should not be processed
        worker.run_once().await.expect("Failed to run worker");

        // The job status should not have changed
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(jobs_after.len(), 1);
        assert_eq!(jobs_after[0].attempts, job.max_attempts);
    })
    .await;
}
