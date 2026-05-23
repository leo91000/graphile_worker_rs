#![cfg(feature = "driver-tokio-postgres")]

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use graphile_worker::sql::add_job::add_job;
use graphile_worker::{DbExecutorArg, DbParams, DbValue, JobSpec};
use serde_json::json;
use tokio_postgres::NoTls;

use helpers::with_test_db;

mod helpers;

fn database_url(database_name: &str) -> String {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let Ok(mut url) = url::Url::parse(&db_url) else {
        return db_url;
    };

    url.set_path(database_name);
    url.to_string()
}

async fn connect_client(database_name: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(&database_url(database_name), NoTls)
        .await
        .expect("Failed to connect tokio-postgres client");

    drop(tokio::spawn(async move {
        let _ = connection.await;
    }));

    client
}

fn deadpool_pool(database_name: &str) -> Pool {
    let config = database_url(database_name)
        .parse::<tokio_postgres::Config>()
        .expect("Failed to parse tokio-postgres config");
    let manager = Manager::from_config(
        config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(2)
        .build()
        .expect("Failed to build deadpool-postgres pool")
}

async fn count_jobs(test_db: &helpers::TestDatabase, identifier: &str) -> i64 {
    sqlx::query_scalar(
        r#"
            SELECT count(*)
            FROM graphile_worker._private_jobs jobs
            JOIN graphile_worker._private_tasks tasks ON tasks.id = jobs.task_id
            WHERE tasks.identifier = $1
        "#,
    )
    .bind(identifier)
    .fetch_one(&test_db.test_pool)
    .await
    .expect("Failed to count jobs")
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
            "graphile_worker",
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
            "graphile_worker",
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
            "graphile_worker",
            "deadpool_pool_job",
            json!({ "pool": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with deadpool pool");

        add_job(
            &client,
            "graphile_worker",
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

#[tokio::test]
async fn tokio_postgres_executor_args_support_direct_queries() {
    with_test_db(|test_db| async move {
        let mut client = connect_client(&test_db.name).await;
        let mut client_executor = &client;

        DbExecutorArg::execute(
            &mut client_executor,
            "SELECT $1::int",
            vec![DbValue::I32(1)].into(),
        )
        .await
        .expect("tokio-postgres client execute should succeed");

        let client_rows = DbExecutorArg::fetch_all(
            &mut client_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(2)].into(),
        )
        .await
        .expect("tokio-postgres client fetch_all should succeed");
        assert_eq!(client_rows[0].try_get::<i32>("value").unwrap(), 2);

        let empty_error = DbExecutorArg::fetch_one(
            &mut client_executor,
            "SELECT $1::int AS value WHERE false",
            vec![DbValue::I32(3)].into(),
        )
        .await
        .expect_err("tokio-postgres client fetch_one should reject empty results");
        assert!(empty_error.to_string().contains("query returned no rows"));

        let tx = client
            .transaction()
            .await
            .expect("Failed to begin tokio-postgres transaction");
        let mut tx_executor = &tx;

        DbExecutorArg::execute(
            &mut tx_executor,
            "SELECT $1::int",
            vec![DbValue::I32(4)].into(),
        )
        .await
        .expect("tokio-postgres transaction execute should succeed");

        let tx_rows = DbExecutorArg::fetch_all(
            &mut tx_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(5)].into(),
        )
        .await
        .expect("tokio-postgres transaction fetch_all should succeed");
        assert_eq!(tx_rows[0].try_get::<i32>("value").unwrap(), 5);

        tx.rollback()
            .await
            .expect("Failed to roll back tokio-postgres transaction");

        let mut tx = client
            .transaction()
            .await
            .expect("Failed to begin mutable tokio-postgres transaction");
        let mut tx_executor = &mut tx;

        DbExecutorArg::execute(
            &mut tx_executor,
            "SELECT $1::int",
            vec![DbValue::I32(6)].into(),
        )
        .await
        .expect("mutable tokio-postgres transaction execute should succeed");

        let tx_rows = DbExecutorArg::fetch_all(
            &mut tx_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(7)].into(),
        )
        .await
        .expect("mutable tokio-postgres transaction fetch_all should succeed");
        assert_eq!(tx_rows[0].try_get::<i32>("value").unwrap(), 7);

        tx.rollback()
            .await
            .expect("Failed to roll back mutable tokio-postgres transaction");

        let pool = deadpool_pool(&test_db.name);
        let mut pool_executor = &pool;

        DbExecutorArg::execute(
            &mut pool_executor,
            "SELECT $1::int",
            vec![DbValue::I32(8)].into(),
        )
        .await
        .expect("deadpool pool execute should succeed");

        let pool_rows = DbExecutorArg::fetch_all(
            &mut pool_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(9)].into(),
        )
        .await
        .expect("deadpool pool fetch_all should succeed");
        assert_eq!(pool_rows[0].try_get::<i32>("value").unwrap(), 9);

        let deadpool_client = pool.get().await.expect("Failed to get deadpool client");
        let mut deadpool_client_executor = &deadpool_client;

        DbExecutorArg::execute(
            &mut deadpool_client_executor,
            "SELECT $1::int",
            vec![DbValue::I32(10)].into(),
        )
        .await
        .expect("deadpool client execute should succeed");

        let deadpool_client_rows = DbExecutorArg::fetch_all(
            &mut deadpool_client_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(11)].into(),
        )
        .await
        .expect("deadpool client fetch_all should succeed");
        assert_eq!(deadpool_client_rows[0].try_get::<i32>("value").unwrap(), 11);

        let params = DbParams::new();
        DbExecutorArg::execute(&mut deadpool_client_executor, "SELECT 1", params)
            .await
            .expect("deadpool client execute without params should succeed");
    })
    .await;
}
