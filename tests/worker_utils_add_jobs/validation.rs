use super::*;

#[tokio::test]
async fn add_jobs_with_empty_list_returns_empty() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_empty";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let empty_jobs: Vec<(BatchJob, &JobSpec)> = vec![];

        let added_jobs = worker
            .create_utils()
            .add_jobs::<BatchJob>(&empty_jobs)
            .await
            .expect("Failed to add empty jobs");

        assert_eq!(added_jobs.len(), 0, "Should return empty vec");

        let db_jobs = test_db.get_jobs().await;
        assert_eq!(db_jobs.len(), 0, "No jobs should be in database");
    })
    .await;
}
#[tokio::test]
async fn add_raw_jobs_with_empty_list_returns_empty() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_raw_empty";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let empty_jobs: Vec<RawJobSpec> = vec![];

        let added_jobs = worker
            .create_utils()
            .add_raw_jobs(&empty_jobs)
            .await
            .expect("Failed to add empty raw jobs");

        assert_eq!(added_jobs.len(), 0, "Should return empty vec");
    })
    .await;
}
#[tokio::test]
async fn add_jobs_rejects_unsafe_dedupe_mode() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_unsafe";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let unsafe_spec = JobSpecBuilder::new()
            .job_key("some_key")
            .job_key_mode(JobKeyMode::UnsafeDedupe)
            .build();

        let jobs_to_add = vec![(BatchJob { value: 1 }, &unsafe_spec)];

        let result = worker
            .create_utils()
            .add_jobs::<BatchJob>(&jobs_to_add)
            .await;

        assert!(result.is_err(), "UnsafeDedupe should fail in batch mode");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("UnsafeDedupe"),
            "Error should mention UnsafeDedupe"
        );
    })
    .await;
}
#[tokio::test]
async fn add_raw_jobs_rejects_unsafe_dedupe_mode() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_raw_unsafe";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let jobs_to_add = vec![RawJobSpec {
            identifier: "batch_job_raw_unsafe".into(),
            payload: serde_json::json!({ "value": 1 }),
            spec: JobSpecBuilder::new()
                .job_key("some_key")
                .job_key_mode(JobKeyMode::UnsafeDedupe)
                .build(),
        }];

        let result = worker.create_utils().add_raw_jobs(&jobs_to_add).await;

        assert!(result.is_err(), "UnsafeDedupe should fail in batch mode");
    })
    .await;
}
