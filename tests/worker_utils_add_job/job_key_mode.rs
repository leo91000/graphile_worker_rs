use chrono::{TimeZone, Utc};
use graphile_worker::{IntoTaskHandlerResult, JobKeyMode, JobSpec, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use crate::helpers::{with_test_db, StaticCounter};

#[tokio::test]
async fn supports_the_job_key_api_with_job_key_mode() {
    with_test_db(|test_db| async move {
        static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

        #[derive(Serialize, Deserialize)]
        struct Job3 {
            a: i32,
        }

        impl TaskHandler for Job3 {
            const IDENTIFIER: &'static str = "job3";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                JOB3_CALL_COUNT.increment().await;
            }
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        let run_at1 = Utc.with_ymd_and_hms(2200, 1, 1, 0, 0, 0).unwrap();
        let run_at2 = Utc.with_ymd_and_hms(2201, 1, 1, 0, 0, 0).unwrap();
        let run_at3 = Utc.with_ymd_and_hms(2202, 1, 1, 0, 0, 0).unwrap();
        let run_at4 = Utc.with_ymd_and_hms(2203, 1, 1, 0, 0, 0).unwrap();

        let j1 = worker
            .create_utils()
            .add_job(
                Job3 { a: 1 },
                JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at1),
                    job_key_mode: Some(JobKeyMode::Replace),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job in replace mode");
        assert_eq!(j1.revision(), &0, "First job revision should be 0");
        assert_eq!(
            j1.run_at(),
            &run_at1,
            "First job run_at should match run_at1"
        );

        let j2 = worker
            .create_utils()
            .add_job(
                Job3 { a: 2 },
                JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at2),
                    job_key_mode: Some(JobKeyMode::PreserveRunAt),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job in preserve_run_at mode");
        assert_eq!(j2.revision(), &1, "Second job revision should be 1");
        assert_eq!(
            j2.run_at(),
            &run_at1,
            "Second job run_at should still match run_at1 due to preserve_run_at mode"
        );

        let j3 = worker
            .create_utils()
            .add_job(
                Job3 { a: 3 },
                JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at3),
                    job_key_mode: Some(JobKeyMode::UnsafeDedupe),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job in unsafe_dedupe mode");
        assert_eq!(j3.revision(), &2, "Third job revision should be 2");
        assert_eq!(
            j3.run_at(),
            &run_at1,
            "Third job run_at should still match run_at1 due to unsafe_dedupe mode"
        );

        let j4 = worker
            .create_utils()
            .add_job(
                Job3 { a: 4 },
                JobSpec {
                    job_key: Some("UNIQUE".into()),
                    run_at: Some(run_at4),
                    job_key_mode: Some(JobKeyMode::Replace),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to replace job");
        assert_eq!(j4.revision(), &3, "Final job revision should be 3");
        assert_eq!(
            j4.run_at(),
            &run_at4,
            "Final job run_at should match run_at4 due to replace mode"
        );

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
