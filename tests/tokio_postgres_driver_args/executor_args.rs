use graphile_worker::{DbExecutorArg, DbParams, DbValue};

use super::support::{connect_client, deadpool_pool};
use crate::helpers::with_test_db;

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
