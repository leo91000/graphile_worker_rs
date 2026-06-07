use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use crate::helpers::{with_test_db, StaticCounter};

#[tokio::test]
async fn supports_the_job_key_api() {
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

        for i in 1..=4 {
            worker
                .create_utils()
                .add_job(
                    Job3 { a: i },
                    JobSpec {
                        job_key: Some("UNIQUE".into()),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job through worker utils");
        }

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

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            1,
            "Job3 should have been executed once"
        );

        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "The job should be removed after execution"
        );
    })
    .await;
}
