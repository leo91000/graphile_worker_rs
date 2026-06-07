use super::*;

#[tokio::test]
async fn context_ext_adds_typed_batch_job_from_task_handler() {
    #[derive(Serialize, Deserialize)]
    struct ParentJob;

    #[derive(Serialize, Deserialize)]
    struct ChildBatchItem {
        id: i32,
    }

    impl TaskHandler for ParentJob {
        const IDENTIFIER: &'static str = "batch_parent_job";

        async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            ctx.add_batch_job(
                vec![ChildBatchItem { id: 1 }, ChildBatchItem { id: 2 }],
                JobSpec::builder()
                    .run_at(chrono::Utc::now() + chrono::Duration::hours(1))
                    .build(),
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
        }
    }

    impl BatchTaskHandler for ChildBatchItem {
        const IDENTIFIER: &'static str = "child_batch_item_from_context";

        async fn run_batch(
            _items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<ParentJob>()
            .define_batch_job::<ChildBatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_job(ParentJob, JobSpec::default())
            .await
            .expect("Failed to add parent job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task_identifier, ChildBatchItem::IDENTIFIER);
        assert_eq!(
            jobs[0].payload,
            json!([
                { "id": 1 },
                { "id": 2 }
            ])
        );
    })
    .await;
}
