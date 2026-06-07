use super::super::*;

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
