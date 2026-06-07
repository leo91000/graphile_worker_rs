use super::*;

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
async fn add_jobs_with_job_key_replaces_locked_job_under_concurrency() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct BatchJob {
            value: i32,
        }

        impl TaskHandler for BatchJob {
            const IDENTIFIER: &'static str = "batch_job_concurrent_dedup";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<BatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let original = worker
            .create_utils()
            .add_job(
                BatchJob { value: 1 },
                JobSpecBuilder::new()
                    .job_key("locked_unique_key")
                    .job_key_mode(JobKeyMode::Replace)
                    .build(),
            )
            .await
            .expect("Failed to add original keyed job");

        safe_query(
            r#"
                UPDATE graphile_worker._private_jobs
                SET locked_by = 'worker-a', locked_at = now()
                WHERE id = $1
            "#,
        )
        .bind(*original.id())
        .execute(&test_db.test_pool)
        .await
        .expect("Failed to lock original keyed job");

        let spec_a = JobSpecBuilder::new()
            .job_key("locked_unique_key")
            .job_key_mode(JobKeyMode::Replace)
            .build();
        let spec_b = JobSpecBuilder::new()
            .job_key("locked_unique_key")
            .job_key_mode(JobKeyMode::Replace)
            .build();
        let jobs_a = [(BatchJob { value: 10 }, &spec_a)];
        let jobs_b = [(BatchJob { value: 20 }, &spec_b)];
        let utils_a = worker.create_utils();
        let utils_b = worker.create_utils();

        let (result_a, result_b) = tokio::join!(
            utils_a.add_jobs::<BatchJob>(&jobs_a),
            utils_b.add_jobs::<BatchJob>(&jobs_b),
        );

        assert_eq!(result_a.expect("first concurrent add_jobs failed").len(), 1);
        assert_eq!(
            result_b.expect("second concurrent add_jobs failed").len(),
            1
        );

        let jobs = test_db.get_jobs().await;
        assert_eq!(
            jobs.iter()
                .filter(|job| job.key.as_deref() == Some("locked_unique_key"))
                .count(),
            1,
            "Only one replacement job should retain the unique key"
        );

        let retired_original = jobs
            .iter()
            .find(|job| job.id == *original.id())
            .expect("Original locked job should remain for normal failure handling");
        assert!(retired_original.key.is_none());
        assert_eq!(retired_original.attempts, retired_original.max_attempts);

        let replacement = jobs
            .iter()
            .find(|job| job.key.as_deref() == Some("locked_unique_key"))
            .expect("Replacement job should keep the unique key");
        let replacement_value = replacement
            .payload
            .get("value")
            .and_then(|value| value.as_i64())
            .expect("Replacement payload should include value");
        assert!(
            replacement_value == 10 || replacement_value == 20,
            "Last writer should win without creating duplicate keyed jobs"
        );
    })
    .await;
}
