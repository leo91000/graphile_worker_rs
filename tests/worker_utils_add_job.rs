use chrono::{TimeZone, Utc};
use graphile_worker::{JobKeyMode, JobSpec, WorkerContext};
use helpers::{with_test_db, StaticCounter};

mod helpers;

#[tokio::test]
async fn runs_a_job_added_through_the_worker_utils() {
    with_test_db(|test_db| async move {
        static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

        #[derive(serde::Deserialize, serde::Serialize)]
        struct Job3Args {
            a: i32,
        }

        #[graphile_worker::task]
        async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
            JOB3_CALL_COUNT.increment().await;
            Ok(())
        }

        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job using worker utilities
        worker
            .create_utils()
            .add_job::<job3>(Job3Args { a: 1 }, None)
            .await
            .expect("Failed to add job through worker utils");

        // Assert that the job has been scheduled
        let jobs_before = test_db.get_jobs().await;
        assert_eq!(jobs_before.len(), 1, "There should be one job scheduled");

        // Run the worker to process the job
        worker.run_once().await.expect("Failed to run worker");

        // Verify the job has been processed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            1,
            "Job3 should have been executed once"
        );

        // Ensure the job has been removed from the queue after execution
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "The job should be removed after execution"
        );
    })
    .await;
}

#[tokio::test]
async fn supports_the_job_key_api() {
    with_test_db(|test_db| async move {
        static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

        #[derive(serde::Deserialize, serde::Serialize)]
        struct Job3Args {
            a: i32,
        }

        #[graphile_worker::task]
        async fn job3(args: Job3Args, _: WorkerContext) -> Result<(), ()> {
            assert_eq!(args.a, 4, "The job payload should have 'a' set to 4");
            JOB3_CALL_COUNT.increment().await;
            Ok(())
        }

        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule multiple jobs with the same jobKey, only the last one should be kept
        for i in 1..=4 {
            worker
                .create_utils()
                .add_job::<job3>(
                    Job3Args { a: i },
                    Some(JobSpec {
                        job_key: Some("UNIQUE".into()),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job through worker utils");
        }

        // Assert that only the most recent job is scheduled
        let jobs = test_db.get_jobs().await;
        assert_eq!(
            jobs.len(),
            1,
            "There should be only one job scheduled due to deduplication"
        );
        assert_eq!(
            jobs[0].payload.get("a").and_then(|v| v.as_i64()),
            Some(4),
            "The scheduled job should have the payload from the last addJob call"
        );
        assert_eq!(
            jobs[0].revision, 3,
            "The job revision should be 3, indicating it was updated three times"
        );

        // Run the worker to process the job
        worker.run_once().await.expect("Failed to run worker");

        // Verify the job has been processed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            1,
            "Job3 should have been executed once"
        );

        // Ensure the job has been removed from the queue after execution
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "The job should be removed after execution"
        );
    })
    .await;
}

#[tokio::test]
async fn supports_the_job_key_api_with_job_key_mode() {
    with_test_db(|test_db| async move {
        static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

        #[derive(serde::Deserialize, serde::Serialize, Debug)]
        struct Job3Args {
            a: i32,
        }

        #[graphile_worker::task]
        async fn job3(_args: Job3Args, _: WorkerContext) -> Result<(), ()> {
            JOB3_CALL_COUNT.increment().await;
            Ok(())
        }

        let worker = test_db
            .create_worker_options()
            .define_job(job3)
            .init()
            .await
            .expect("Failed to create worker");

        let run_at1 = Utc.with_ymd_and_hms(2200, 1, 1, 0, 0, 0).unwrap();
        let run_at2 = Utc.with_ymd_and_hms(2201, 1, 1, 0, 0, 0).unwrap();
        let run_at3 = Utc.with_ymd_and_hms(2202, 1, 1, 0, 0, 0).unwrap();
        let run_at4 = Utc.with_ymd_and_hms(2203, 1, 1, 0, 0, 0).unwrap();

        // Job first added in replace mode
        let j1 = worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 1 },
                Some(JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at1),
                    job_key_mode: Some(JobKeyMode::Replace),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job in replace mode");
        assert_eq!(j1.revision(), &0, "First job revision should be 0");
        assert_eq!(
            j1.run_at(),
            &run_at1,
            "First job run_at should match run_at1"
        );

        // Now updated, but preserve run_at
        let j2 = worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 2 },
                Some(JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at2),
                    job_key_mode: Some(JobKeyMode::PreserveRunAt),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job in preserve_run_at mode");
        assert_eq!(j2.revision(), &1, "Second job revision should be 1");
        assert_eq!(
            j2.run_at(),
            &run_at1,
            "Second job run_at should still match run_at1 due to preserve_run_at mode"
        );

        // Unsafe dedupe should take no action other than to bump the revision number
        let j3 = worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 3 },
                Some(JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at3),
                    job_key_mode: Some(JobKeyMode::UnsafeDedupe),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job in unsafe_dedupe mode");
        assert_eq!(j3.revision(), &2, "Third job revision should be 2");
        assert_eq!(
            j3.run_at(),
            &run_at1,
            "Third job run_at should still match run_at1 due to unsafe_dedupe mode"
        );

        // Replace the job one final time
        let j4 = worker
            .create_utils()
            .add_job::<job3>(
                Job3Args { a: 4 },
                Some(JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at4),
                    job_key_mode: Some(JobKeyMode::Replace),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to replace job");
        assert_eq!(j4.revision(), &3, "Final job revision should be 3");
        assert_eq!(
            j4.run_at(),
            &run_at4,
            "Final job run_at should match run_at4 due to replace mode"
        );

        // Ensure the job has been removed from the queue after execution
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            1,
            "The job should be removed after execution"
        );
        let job = &jobs_after[0];
        assert_eq!(job.revision, 3, "The job revision should be 3");
        assert_eq!(
            job.payload.get("a").and_then(|v| v.as_i64()),
            Some(4),
            "The job payload should have 'a' set to 4"
        );
        assert_eq!(job.run_at, run_at4, "The job run_at should match run_at4");
    })
    .await;
}
