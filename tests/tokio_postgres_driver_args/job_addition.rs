use graphile_worker::sql::add_job::single::add_job;
use graphile_worker::{JobSpec, TaskHandler};
use serde_json::json;

use super::support::{connect_client, count_jobs, deadpool_pool};
use crate::helpers::with_test_db;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct TransactionalWorkerUtilsJob {
    value: i32,
}

impl TaskHandler for TransactionalWorkerUtilsJob {
    const IDENTIFIER: &'static str = "tokio_postgres_worker_utils_transaction_job";

    async fn run(
        self,
        _ctx: graphile_worker::WorkerContext,
    ) -> impl graphile_worker::IntoTaskHandlerResult {
    }
}

#[tokio::test]
async fn tokio_postgres_client_executor_adds_job() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let client = connect_client(&test_db.name).await;

        add_job(
            &client,
            &graphile_worker::Schema::default(),
            "tokio_postgres_client_job",
            json!({ "client": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with tokio-postgres client");

        assert_eq!(count_jobs(&test_db, "tokio_postgres_client_job").await, 1);
    })
    .await;
}

#[tokio::test]
async fn tokio_postgres_transaction_executor_participates_in_caller_transaction() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut client = connect_client(&test_db.name).await;
        let tx = client
            .transaction()
            .await
            .expect("Failed to begin tokio-postgres transaction");

        add_job(
            &tx,
            &graphile_worker::Schema::default(),
            "tokio_postgres_transaction_job",
            json!({ "transactional": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with tokio-postgres transaction");

        tx.rollback()
            .await
            .expect("Failed to roll back tokio-postgres transaction");

        assert_eq!(
            count_jobs(&test_db, "tokio_postgres_transaction_job").await,
            0
        );
    })
    .await;
}

#[tokio::test]
async fn worker_utils_routes_jobs_through_tokio_postgres_transaction() {
    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<TransactionalWorkerUtilsJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let mut client = connect_client(&test_db.name).await;
        let tx = client
            .transaction()
            .await
            .expect("Failed to begin tokio-postgres transaction");
        let mut utils = worker.create_utils().with_executor(&tx);
        let spec = JobSpec::default();
        let jobs = [
            (TransactionalWorkerUtilsJob { value: 1 }, &spec),
            (TransactionalWorkerUtilsJob { value: 2 }, &spec),
        ];

        let added = utils
            .add_jobs(&jobs)
            .await
            .expect("Failed to add jobs through WorkerUtils transaction");
        assert_eq!(added.len(), 2);

        let _utils = utils.into_inner();
        tx.rollback()
            .await
            .expect("Failed to roll back tokio-postgres transaction");

        assert_eq!(
            count_jobs(&test_db, TransactionalWorkerUtilsJob::IDENTIFIER).await,
            0
        );
    })
    .await;
}

#[tokio::test]
async fn deadpool_postgres_pool_and_client_executors_add_jobs() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let pool = deadpool_pool(&test_db.name);
        let client = pool.get().await.expect("Failed to get deadpool client");

        add_job(
            &pool,
            &graphile_worker::Schema::default(),
            "deadpool_pool_job",
            json!({ "pool": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with deadpool pool");

        add_job(
            &client,
            &graphile_worker::Schema::default(),
            "deadpool_client_job",
            json!({ "client": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with deadpool client");

        assert_eq!(count_jobs(&test_db, "deadpool_pool_job").await, 1);
        assert_eq!(count_jobs(&test_db, "deadpool_client_job").await, 1);
    })
    .await;
}
