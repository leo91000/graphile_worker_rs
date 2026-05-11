use graphile_worker::{
    BatchTaskHandler, IntoBatchTaskHandlerResult, JobSpec, WorkerContext, WorkerOptions,
};
use helpers::{with_test_db, StaticCounter};
use serde::{Deserialize, Serialize};
use serde_json::json;

mod helpers;

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
        let worker = WorkerOptions::default()
            .database(test_db.database.clone())
            .schema("graphile_worker")
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
