use super::*;

#[tokio::test]
async fn runs_typed_batch_job_to_completion() {
    static ITEM_COUNT: StaticCounter = StaticCounter::new();
    ITEM_COUNT.reset().await;

    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "batch_item";

        async fn run_batch(
            items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            for _item in items {
                ITEM_COUNT.increment().await;
            }
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
                vec![
                    BatchItem { id: 1 },
                    BatchItem { id: 2 },
                    BatchItem { id: 3 },
                ],
                JobSpec::default(),
            )
            .await
            .expect("Failed to add batch job");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].payload,
            json!([
                { "id": 1 },
                { "id": 2 },
                { "id": 3 }
            ])
        );

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(ITEM_COUNT.get().await, 3);
        assert!(test_db.get_jobs().await.is_empty());
    })
    .await;
}
