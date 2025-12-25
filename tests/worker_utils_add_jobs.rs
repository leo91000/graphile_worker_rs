use graphile_worker::{
    IntoTaskHandlerResult, JobKeyMode, JobSpec, JobSpecBuilder, RawJobSpec, TaskHandler,
    WorkerContext,
};
use helpers::{with_test_db, StaticCounter};
use serde::{Deserialize, Serialize};

mod helpers;

#[tokio::test]
async fn add_jobs_inserts_multiple_jobs_of_same_type() {
    with_test_db(|test_db| async move {
        static COUNTER: StaticCounter = StaticCounter::new();

        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_insert";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                COUNTER.increment().await;
            }
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let spec = JobSpec::default();
        let jobs_to_add = vec![
            (BatchJob { value: 1 }, &spec),
            (BatchJob { value: 2 }, &spec),
            (BatchJob { value: 3 }, &spec),
        ];

        let added_jobs = worker
            .create_utils()
            .add_jobs::<BatchJob>(&jobs_to_add)
            .await
            .expect("Failed to add jobs");

        assert_eq!(added_jobs.len(), 3, "Should have added 3 jobs");

        let db_jobs = test_db.get_jobs().await;
        assert_eq!(db_jobs.len(), 3, "Should have 3 jobs in database");

        let payloads: Vec<i32> = db_jobs
            .iter()
            .map(|j| j.payload.get("value").unwrap().as_i64().unwrap() as i32)
            .collect();
        assert!(payloads.contains(&1));
        assert!(payloads.contains(&2));
        assert!(payloads.contains(&3));

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(
            COUNTER.get().await,
            3,
            "All 3 jobs should have been executed"
        );

        let jobs_after = test_db.get_jobs().await;
        assert_eq!(jobs_after.len(), 0, "All jobs should be processed");
    })
    .await;
}

#[tokio::test]
async fn add_jobs_with_shared_spec() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_shared_spec";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let spec = JobSpecBuilder::new()
            .priority(10)
            .queue_name("batch_queue")
            .build();

        let jobs_to_add = vec![
            (BatchJob { value: 100 }, &spec),
            (BatchJob { value: 200 }, &spec),
        ];

        worker
            .create_utils()
            .add_jobs::<BatchJob>(&jobs_to_add)
            .await
            .expect("Failed to add jobs");

        let db_jobs = test_db.get_jobs().await;
        assert_eq!(db_jobs.len(), 2);

        for job in &db_jobs {
            assert_eq!(job.priority, 10, "Priority should be 10");
            assert_eq!(
                job.queue_name.as_deref(),
                Some("batch_queue"),
                "Queue name should match"
            );
        }
    })
    .await;
}

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

#[tokio::test]
async fn add_jobs_with_job_key_deduplication() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_dedup";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let spec_with_key = JobSpecBuilder::new()
            .job_key("unique_key")
            .job_key_mode(JobKeyMode::Replace)
            .build();

        let spec_no_key = JobSpec::default();

        let jobs_to_add = vec![
            (BatchJob { value: 1 }, &spec_with_key),
            (BatchJob { value: 2 }, &spec_no_key),
            (BatchJob { value: 3 }, &spec_no_key),
        ];

        worker
            .create_utils()
            .add_jobs::<BatchJob>(&jobs_to_add)
            .await
            .expect("Failed to add first batch");

        let jobs_after_first = test_db.get_jobs().await;
        assert_eq!(jobs_after_first.len(), 3);

        let spec_with_same_key = JobSpecBuilder::new()
            .job_key("unique_key")
            .job_key_mode(JobKeyMode::Replace)
            .build();

        let second_batch = vec![(BatchJob { value: 100 }, &spec_with_same_key)];

        worker
            .create_utils()
            .add_jobs::<BatchJob>(&second_batch)
            .await
            .expect("Failed to add second batch");

        let jobs_after_second = test_db.get_jobs().await;
        assert_eq!(
            jobs_after_second.len(),
            3,
            "Should still have 3 jobs (key deduplicated)"
        );

        let keyed_job = jobs_after_second
            .iter()
            .find(|j| j.key.as_deref() == Some("unique_key"))
            .expect("Should find job with key");

        assert_eq!(
            keyed_job.payload.get("value").unwrap().as_i64(),
            Some(100),
            "Keyed job should have updated payload"
        );
        assert_eq!(keyed_job.revision, 1, "Revision should be 1 after update");
    })
    .await;
}

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

#[tokio::test]
async fn add_jobs_preserves_individual_specs() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_individual";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let spec1 = JobSpecBuilder::new().priority(1).build();
        let spec2 = JobSpecBuilder::new().priority(10).build();
        let spec3 = JobSpecBuilder::new().priority(5).build();

        let jobs_to_add = vec![
            (BatchJob { value: 1 }, &spec1),
            (BatchJob { value: 2 }, &spec2),
            (BatchJob { value: 3 }, &spec3),
        ];

        worker
            .create_utils()
            .add_jobs::<BatchJob>(&jobs_to_add)
            .await
            .expect("Failed to add jobs");

        let db_jobs = test_db.get_jobs().await;
        assert_eq!(db_jobs.len(), 3);

        let job1 = db_jobs
            .iter()
            .find(|j| j.payload.get("value").unwrap().as_i64() == Some(1))
            .unwrap();
        let job2 = db_jobs
            .iter()
            .find(|j| j.payload.get("value").unwrap().as_i64() == Some(2))
            .unwrap();
        let job3 = db_jobs
            .iter()
            .find(|j| j.payload.get("value").unwrap().as_i64() == Some(3))
            .unwrap();

        assert_eq!(job1.priority, 1);
        assert_eq!(job2.priority, 10);
        assert_eq!(job3.priority, 5);
    })
    .await;
}
