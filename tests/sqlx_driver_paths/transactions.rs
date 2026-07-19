use super::*;
use graphile_worker::{BatchTaskHandler, TaskHandler};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct TransactionalJob {
    value: i32,
}

impl graphile_worker::TaskHandler for TransactionalJob {
    const IDENTIFIER: &'static str = "worker_utils_transactional_job";

    async fn run(
        self,
        _ctx: graphile_worker::WorkerContext,
    ) -> impl graphile_worker::IntoTaskHandlerResult {
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct TransactionalBatchJob {
    value: i32,
}

impl graphile_worker::BatchTaskHandler for TransactionalBatchJob {
    const IDENTIFIER: &'static str = "worker_utils_transactional_batch_job";

    async fn run_batch(
        _items: Vec<Self>,
        _ctx: graphile_worker::WorkerContext,
    ) -> impl graphile_worker::IntoBatchTaskHandlerResult {
    }
}

#[tokio::test]
async fn sqlx_transaction_executor_participates_in_caller_transaction() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");

        add_job(
            &mut tx,
            &graphile_worker::Schema::default(),
            "sqlx_transaction_job",
            json!({ "transactional": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job in SQLx transaction");

        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        assert_eq!(count_jobs(&test_db, "sqlx_transaction_job").await, 0);
    })
    .await;
}
#[tokio::test]
async fn sqlx_transaction_executor_handles_batch_and_multistep_helpers() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");

        let task_details = get_tasks_details(
            &mut tx,
            &graphile_worker::Schema::default(),
            vec!["sqlx_transaction_batch".to_string()],
        )
        .await
        .expect("Failed to get task details in transaction");

        let spec = JobSpec::default();
        let jobs = [
            JobToAdd {
                identifier: "sqlx_transaction_batch",
                payload: json!({ "value": 1 }),
                spec: &spec,
            },
            JobToAdd {
                identifier: "sqlx_transaction_batch",
                payload: json!({ "value": 2 }),
                spec: &spec,
            },
        ];

        let added = add_jobs(
            &mut tx,
            &graphile_worker::Schema::default(),
            &jobs,
            &task_details,
            false,
            false,
        )
        .await
        .expect("Failed to add batch jobs in SQLx transaction");
        assert_eq!(added.len(), 2);

        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        assert_eq!(count_jobs(&test_db, "sqlx_transaction_batch").await, 0);
    })
    .await;
}

#[tokio::test]
async fn worker_utils_routes_operations_through_sqlx_transaction() {
    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<TransactionalJob>()
            .define_batch_job::<TransactionalBatchJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");
        let mut utils = worker.create_utils().with_executor(&mut tx);

        let spec = JobSpec::default();
        let jobs = [
            (TransactionalJob { value: 1 }, &spec),
            (TransactionalJob { value: 2 }, &spec),
            (TransactionalJob { value: 3 }, &spec),
        ];
        let added = utils
            .add_jobs(&jobs)
            .await
            .expect("Failed to add typed jobs through WorkerUtils transaction");
        assert_eq!(added.len(), 3);
        assert!(added
            .iter()
            .all(|job| job.task_identifier() == TransactionalJob::IDENTIFIER));

        let rescheduled = utils
            .reschedule_jobs(
                &[*added[0].id()],
                graphile_worker::worker_utils::types::RescheduleJobOptions::default(),
            )
            .await
            .expect("Failed to reschedule job through WorkerUtils transaction");
        assert_eq!(rescheduled.len(), 1);

        let completed = utils
            .complete_jobs(&[*added[1].id()])
            .await
            .expect("Failed to complete job through WorkerUtils transaction");
        assert_eq!(completed.len(), 1);

        let failed = utils
            .permanently_fail_jobs(&[*added[2].id()], "transaction test")
            .await
            .expect("Failed to fail job through WorkerUtils transaction");
        assert_eq!(failed.len(), 1);

        utils
            .add_job(
                TransactionalJob { value: 4 },
                JobSpecBuilder::new()
                    .job_key("worker-utils-transaction-key")
                    .build(),
            )
            .await
            .expect("Failed to add keyed job through WorkerUtils transaction");
        utils
            .remove_job("worker-utils-transaction-key")
            .await
            .expect("Failed to remove keyed job through WorkerUtils transaction");

        let raw_jobs = [graphile_worker::RawJobSpec {
            identifier: "worker_utils_transactional_raw_job".to_string(),
            payload: json!({ "value": 5 }),
            spec: JobSpec::default(),
        }];
        let raw_added = utils
            .add_raw_jobs(&raw_jobs)
            .await
            .expect("Failed to add raw jobs through WorkerUtils transaction");
        assert_eq!(raw_added[0].task_identifier(), &raw_jobs[0].identifier);

        utils
            .add_batch_job(vec![TransactionalBatchJob { value: 6 }], JobSpec::default())
            .await
            .expect("Failed to add batch job through WorkerUtils transaction");

        utils
            .list_active_workers(std::time::Duration::from_secs(60))
            .await
            .expect("Failed to list workers through WorkerUtils transaction");
        utils
            .force_unlock_workers(&[])
            .await
            .expect("Failed to force unlock workers through WorkerUtils transaction");

        let _utils = utils.into_inner();
        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        assert_eq!(count_jobs(&test_db, TransactionalJob::IDENTIFIER).await, 0);
        assert_eq!(
            count_jobs(&test_db, "worker_utils_transactional_raw_job").await,
            0
        );
        assert_eq!(
            count_jobs(&test_db, TransactionalBatchJob::IDENTIFIER).await,
            0
        );
    })
    .await;
}
