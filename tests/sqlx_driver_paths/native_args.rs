use super::*;

#[tokio::test]
async fn sqlx_connection_executor_adds_job() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut connection = test_db
            .test_pool
            .acquire()
            .await
            .expect("Failed to acquire SQLx connection");

        add_job(
            &mut *connection,
            &graphile_worker::Schema::default(),
            "sqlx_connection_job",
            json!({ "connection": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with SQLx connection");

        assert_eq!(count_jobs(&test_db, "sqlx_connection_job").await, 1);
    })
    .await;
}
#[tokio::test]
async fn sqlx_native_executor_args_support_direct_queries() {
    with_test_db(|test_db| async move {
        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");
        let mut tx_executor = &mut tx;

        DbExecutorArg::execute(
            &mut tx_executor,
            "SELECT $1::int",
            vec![DbValue::I32(1)].into(),
        )
        .await
        .expect("SQLx transaction execute should succeed");

        let tx_rows = DbExecutorArg::fetch_all(
            &mut tx_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(2)].into(),
        )
        .await
        .expect("SQLx transaction fetch_all should succeed");
        assert_eq!(tx_rows[0].try_get::<i32>("value").unwrap(), 2);

        let empty_error = DbExecutorArg::fetch_one(
            &mut tx_executor,
            "SELECT $1::int AS value WHERE false",
            vec![DbValue::I32(3)].into(),
        )
        .await
        .expect_err("SQLx transaction fetch_one should reject empty results");
        assert!(empty_error.to_string().contains("query returned no rows"));

        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        let mut connection = test_db
            .test_pool
            .acquire()
            .await
            .expect("Failed to acquire SQLx connection");
        let mut connection_executor = &mut *connection;

        DbExecutorArg::execute(
            &mut connection_executor,
            "SELECT $1::int",
            vec![DbValue::I32(4)].into(),
        )
        .await
        .expect("SQLx connection execute should succeed");

        let connection_rows = DbExecutorArg::fetch_all(
            &mut connection_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(5)].into(),
        )
        .await
        .expect("SQLx connection fetch_all should succeed");
        assert_eq!(connection_rows[0].try_get::<i32>("value").unwrap(), 5);

        let mut pool_executor = &test_db.test_pool;
        let pool_rows = DbExecutorArg::fetch_all(
            &mut pool_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(6)].into(),
        )
        .await
        .expect("SQLx pool DbExecutorArg fetch_all should succeed");
        assert_eq!(pool_rows[0].try_get::<i32>("value").unwrap(), 6);

        let params = DbParams::new();
        DbExecutorArg::execute(&mut pool_executor, "SELECT 1", params)
            .await
            .expect("SQLx pool DbExecutorArg execute should succeed");
    })
    .await;
}
