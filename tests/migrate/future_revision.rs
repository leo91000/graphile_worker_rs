use super::*;

#[tokio::test]
async fn aborts_if_database_is_more_up_to_date_than_current_worker() {
    with_test_db(|test_db| async move {
        // Drop existing schema to ensure a clean state
        query("DROP SCHEMA IF EXISTS graphile_worker CASCADE;")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        // Perform initial migration
        migrate(&test_db.database, "graphile_worker")
            .await
            .expect("Failed to perform initial migration");

        // Insert a more up-to-date migration to simulate a future schema version
        query("INSERT INTO graphile_worker.migrations (id, ts, breaking) VALUES (999999, '2023-10-19T10:31:00Z', true);")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        // Attempt to migrate again and expect it to fail due to version incompatibility
        let migration_result = migrate(&test_db.database, "graphile_worker").await;

        assert!(
            matches!(
                migration_result,
                Err(MigrateError::IncompatbleRevision { latest_migration, latest_breaking_migration, highest_migration })
                if latest_migration == 999999 && latest_breaking_migration == 999999 && highest_migration == 20
            ),
            "Expected migration to abort due to database being more up to date than current worker"
        );
    })
    .await;
}
