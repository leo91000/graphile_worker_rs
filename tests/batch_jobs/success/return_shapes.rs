use super::*;

#[tokio::test]
async fn batch_handlers_support_successful_return_shapes() {
    #[derive(Serialize, Deserialize)]
    struct ResultOkBatchItem {
        id: i32,
    }

    impl BatchTaskHandler for ResultOkBatchItem {
        const IDENTIFIER: &'static str = "result_ok_batch_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            Ok::<(), String>(())
        }
    }

    #[derive(Serialize, Deserialize)]
    struct BatchResultCompleteItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchResultCompleteItem {
        const IDENTIFIER: &'static str = "batch_result_complete_item";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            BatchTaskResult::<String>::Complete
        }
    }

    #[derive(Serialize, Deserialize)]
    struct ItemResultsOkBatchItem {
        id: i32,
    }

    impl BatchTaskHandler for ItemResultsOkBatchItem {
        const IDENTIFIER: &'static str = "item_results_ok_batch_item";

        async fn run_batch(
            items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            BatchTaskResult::ItemResults(vec![Ok::<(), String>(()); items.len()])
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_batch_job::<ResultOkBatchItem>()
            .define_batch_job::<BatchResultCompleteItem>()
            .define_batch_job::<ItemResultsOkBatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_batch_job(vec![ResultOkBatchItem { id: 1 }], JobSpec::default())
            .await
            .expect("Failed to add result-ok batch job");
        worker
            .create_utils()
            .add_batch_job(vec![BatchResultCompleteItem { id: 2 }], JobSpec::default())
            .await
            .expect("Failed to add batch-result-complete batch job");
        worker
            .create_utils()
            .add_batch_job(
                vec![
                    ItemResultsOkBatchItem { id: 3 },
                    ItemResultsOkBatchItem { id: 4 },
                ],
                JobSpec::default(),
            )
            .await
            .expect("Failed to add item-results-ok batch job");

        worker.run_once().await.expect("Failed to run worker");

        assert!(test_db.get_jobs().await.is_empty());
    })
    .await;
}
