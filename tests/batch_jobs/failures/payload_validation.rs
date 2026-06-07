use super::super::*;

#[tokio::test]
async fn batch_handler_requires_array_payload() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "array_payload_batch_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
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
            .add_raw_job(
                BatchItem::IDENTIFIER,
                json!({ "id": 1 }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add raw job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].payload, json!({ "id": 1 }));
        assert!(jobs[0]
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("batch job payload must be a JSON array")));
    })
    .await;
}

#[tokio::test]
async fn batch_handler_reports_item_deserialization_errors() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "invalid_item_payload_batch_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
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
            .add_raw_job(
                BatchItem::IDENTIFIER,
                json!([{ "id": "not-an-integer" }]),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add raw job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].payload, json!([{ "id": "not-an-integer" }]));
        assert!(jobs[0]
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("invalid type")));
    })
    .await;
}
