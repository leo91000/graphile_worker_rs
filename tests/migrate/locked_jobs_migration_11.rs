use super::*;

#[tokio::test]
async fn throws_helpful_error_message_in_migration_11() {
    with_test_db(|test_db| async move {
        // Drop existing schema to ensure a clean state
        query("CREATE SCHEMA IF NOT EXISTS graphile_worker;")
            .execute(&test_db.test_pool)
            .await
            .unwrap();
        query("CREATE TABLE IF NOT EXISTS graphile_worker.migrations(id INT PRIMARY KEY, ts TIMESTAMPTZ DEFAULT NOW() NOT NULL, breaking BOOLEAN DEFAULT FALSE NOT NULL);")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        // Manually run the first 10 migrations
        let migrations = &GRAPHILE_WORKER_MIGRATIONS[..10];
        let tx = test_db.database.begin().await.unwrap();
        for migration in migrations {
            migration.execute(&tx, "graphile_worker").await.unwrap();
            let sql = "insert into graphile_worker.migrations (id, breaking) values ($1, $2)";
            tx.execute(
                sql,
                vec![
                    DbValue::I32(migration.migration_number() as i32),
                    DbValue::Bool(migration.is_breaking()),
                ]
                .into(),
            )
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();

        // Lock a job
        query("SELECT graphile_worker.add_job('lock_me', '{}');")
            .execute(&test_db.test_pool)
            .await
            .unwrap();
        query("UPDATE graphile_worker.jobs SET locked_at = NOW(), locked_by = 'test_runner';")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        // Attempt to perform migration
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let migration_result = migrate(&test_db.database, "graphile_worker").await;

        assert!(
            matches!(migration_result, Err(MigrateError::LockedJobInMigration11)),
            "Expected migration to fail due to locked jobs"
        );
    })
    .await;
}
