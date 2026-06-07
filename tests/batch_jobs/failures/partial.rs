use super::super::*;

#[tokio::test]
async fn partial_batch_failure_retries_only_failed_items() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
        fail: bool,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "partial_batch_item";

        async fn run_batch(
            items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            items
                .into_iter()
                .map(|item| {
                    if item.fail {
                        Err(format!("failed {}", item.id))
                    } else {
                        Ok(())
                    }
                })
                .collect::<Vec<_>>()
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
                    BatchItem { id: 1, fail: false },
                    BatchItem { id: 2, fail: true },
                    BatchItem { id: 3, fail: false },
                    BatchItem { id: 4, fail: true },
                ],
                JobSpec::default(),
            )
            .await
            .expect("Failed to add batch job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].attempts, 1);
        assert_eq!(
            jobs[0].payload,
            json!([
                { "id": 2, "fail": true },
                { "id": 4, "fail": true }
            ])
        );
        assert!(jobs[0]
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("2 batch item(s) failed")));
        assert_eq!(jobs[0].locked_at, None);
        assert_eq!(jobs[0].locked_by, None);
    })
    .await;
}

#[tokio::test]
async fn partial_batch_failure_preserves_payload_with_failure_batcher() {
    #[derive(Serialize, Deserialize)]
    struct BatchItem {
        id: i32,
        fail: bool,
    }

    impl BatchTaskHandler for BatchItem {
        const IDENTIFIER: &'static str = "batched_partial_failure_batcher_item";

        async fn run_batch(
            items: Vec<Self>,
            _ctx: WorkerContext,
        ) -> impl IntoBatchTaskHandlerResult {
            items
                .into_iter()
                .map(|item| {
                    if item.fail {
                        Err(format!("failed {}", item.id))
                    } else {
                        Ok(())
                    }
                })
                .collect::<Vec<_>>()
        }
    }

    with_test_db(|test_db| async move {
        let plugin = BatchFailureHookPlugin::default();
        let hook_counts = plugin.clone();
        let worker = test_db
            .create_worker_options()
            .fail_job_batch_delay(Duration::from_millis(10))
            .add_plugin(plugin)
            .define_batch_job::<BatchItem>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_batch_job(
                vec![
                    BatchItem { id: 1, fail: false },
                    BatchItem { id: 2, fail: true },
                ],
                JobSpec::default(),
            )
            .await
            .expect("Failed to add retryable batch job");
        worker
            .create_utils()
            .add_batch_job(
                vec![BatchItem { id: 3, fail: true }],
                JobSpec::builder().max_attempts(1).build(),
            )
            .await
            .expect("Failed to add permanent batch job");

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(hook_counts.fail_count(), 1);
        assert_eq!(hook_counts.permanent_count(), 1);

        let jobs = test_db.get_jobs().await;
        assert!(jobs.iter().any(|job| {
            job.payload
                == json!([
                    { "id": 2, "fail": true }
                ])
        }));
    })
    .await;
}
