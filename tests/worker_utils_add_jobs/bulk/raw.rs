use super::super::*;

#[tokio::test]
async fn add_raw_jobs_inserts_heterogeneous_jobs() {
    with_test_db(|test_db| async move {
        static BATCH_COUNTER: StaticCounter = StaticCounter::new();
        static OTHER_COUNTER: StaticCounter = StaticCounter::new();

        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_hetero";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                BATCH_COUNTER.increment().await;
            }
        }

        #[derive(Clone, Serialize, Deserialize)]
        struct OtherJob {
            name: String,
        }

        impl TaskHandler for OtherJob {
            const IDENTIFIER: &'static str = "other_job_hetero";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                OTHER_COUNTER.increment().await;
            }
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .define_job::<OtherJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let jobs_to_add = vec![
            RawJobSpec {
                identifier: "batch_job_hetero".into(),
                payload: serde_json::json!({ "value": 42 }),
                spec: JobSpec::default(),
            },
            RawJobSpec {
                identifier: "other_job_hetero".into(),
                payload: serde_json::json!({ "name": "test" }),
                spec: JobSpec::default(),
            },
            RawJobSpec {
                identifier: "batch_job_hetero".into(),
                payload: serde_json::json!({ "value": 99 }),
                spec: JobSpecBuilder::new().priority(5).build(),
            },
        ];

        let added_jobs = worker
            .create_utils()
            .add_raw_jobs(&jobs_to_add)
            .await
            .expect("Failed to add raw jobs");

        assert_eq!(added_jobs.len(), 3, "Should have added 3 jobs");

        let db_jobs = test_db.get_jobs().await;
        assert_eq!(db_jobs.len(), 3, "Should have 3 jobs in database");

        let batch_jobs: Vec<_> = db_jobs
            .iter()
            .filter(|j| j.task_identifier == "batch_job_hetero")
            .collect();
        let other_jobs: Vec<_> = db_jobs
            .iter()
            .filter(|j| j.task_identifier == "other_job_hetero")
            .collect();

        assert_eq!(batch_jobs.len(), 2);
        assert_eq!(other_jobs.len(), 1);

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(BATCH_COUNTER.get().await, 2, "BatchJob should run 2 times");
        assert_eq!(OTHER_COUNTER.get().await, 1, "OtherJob should run 1 time");
    })
    .await;
}
