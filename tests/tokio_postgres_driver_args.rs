#![cfg(feature = "driver-tokio-postgres")]

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use graphile_worker::sql::add_job::add_job;
use graphile_worker::JobSpec;
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
