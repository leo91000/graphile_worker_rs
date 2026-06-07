use super::super::*;

#[tokio::test]
async fn mismatched_batch_result_length_retries_original_payload() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "mismatched_batch_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            vec![Ok::<(), String>(())]
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_batch_job::<BatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_batch_job(
                vec![BatchItem { id: 1 }, BatchItem { id: 2 }],
                JobSpec::default(),
            )
            .await
            .expect("Failed to add batch job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].payload,
            json!([
                { "id": 1 },
                { "id": 2 }
            ])
        );
        assert!(jobs[0]
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("returned 1 results for 2 payload items")));
    })
    .await;
}

#[tokio::test]
async fn whole_batch_failure_retries_original_payload() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "failing_batch_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            Err::<(), _>("batch failed")
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_batch_job::<BatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_batch_job(
                vec![BatchItem { id: 1 }, BatchItem { id: 2 }],
                JobSpec::default(),
            )
            .await
            .expect("Failed to add batch job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].payload,
            json!([
                { "id": 1 },
                { "id": 2 }
            ])
        );
        assert!(jobs[0]
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("batch failed")));
    })
    .await;
}
