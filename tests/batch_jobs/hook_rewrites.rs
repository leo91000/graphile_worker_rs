use super::*;

#[tokio::test]
async fn rejects_batch_payload_rewritten_to_object_by_hook() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "batch_hook_object_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .add_plugin(BatchPayloadOverridePlugin {
                payload: json!({ "id": 1 }),
            })
            .define_batch_job::<BatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        let result = worker
            .create_utils()
            .add_batch_job(vec![BatchItem { id: 1 }], JobSpec::default())
            .await;

        assert!(result
            .err()
            .is_some_and(|error| error.to_string().contains("JSON array")));
        assert!(test_db.get_jobs().await.is_empty());
    })
    .await;
}
#[tokio::test]
async fn rejects_batch_payload_rewritten_to_empty_array_by_hook() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "batch_hook_empty_array_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .add_plugin(BatchPayloadOverridePlugin { payload: json!([]) })
            .define_batch_job::<BatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        let result = worker
            .create_utils()
            .add_batch_job(vec![BatchItem { id: 1 }], JobSpec::default())
            .await;

        assert!(result
            .err()
            .is_some_and(|error| error.to_string().contains("at least one item")));
        assert!(test_db.get_jobs().await.is_empty());
    })
    .await;
}
