use super::*;

#[tokio::test]
async fn rejects_empty_typed_batch_payloads() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "empty_batch_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
        }
    }

    with_test_db(|test_db| async move {
        let result = test_db
            .worker_utils()
            .add_batch_job::<BatchItem>(vec![], JobSpec::default())
            .await;

        assert!(result.is_err());
        assert!(result
            .err()
            .is_some_and(|error| error.to_string().contains("at least one item")));
    })
    .await;
}
