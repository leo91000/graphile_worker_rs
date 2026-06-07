use super::*;

#[cfg(feature = "driver-sqlx")]
#[tokio::test]
async fn migration_execute_supports_sqlx_transaction() {
    with_test_db(|test_db| async move {
        query("DROP SCHEMA IF EXISTS graphile_worker CASCADE;")
            .execute(&test_db.test_pool)
            .await
            .unwrap();
        query("CREATE SCHEMA graphile_worker;")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        let migration = GraphileWorkerMigration {
            name: "m000001_sqlx_transaction",
            is_breaking: false,
            stmts: MigrationStatements::Static(&[
                "CREATE TABLE :GRAPHILE_WORKER_SCHEMA.sqlx_transaction_migration_test(id INT PRIMARY KEY)",
                "INSERT INTO :GRAPHILE_WORKER_SCHEMA.sqlx_transaction_migration_test(id) VALUES (1)",
            ]),
        };

        let mut tx = test_db.test_pool.begin().await.unwrap();
        migration.execute(&mut tx, "graphile_worker").await.unwrap();
        tx.commit().await.unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM graphile_worker.sqlx_transaction_migration_test")
                .fetch_one(&test_db.test_pool)
                .await
                .unwrap();

        assert_eq!(count, 1);
    })
    .await;
}
