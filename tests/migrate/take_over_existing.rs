use super::*;

#[tokio::test]
async fn migration_can_take_over_from_pre_existing_migrations_table() {
    with_test_db(|test_db| async move {
        // Drop existing schema and create a pre-existing migrations table
        let initial_stmts = &[
            "DROP SCHEMA IF EXISTS graphile_worker CASCADE",
            "CREATE SCHEMA IF NOT EXISTS graphile_worker",
            "CREATE TABLE IF NOT EXISTS graphile_worker.migrations(id INT PRIMARY KEY, ts TIMESTAMPTZ DEFAULT NOW() NOT NULL, breaking BOOLEAN DEFAULT FALSE NOT NULL)",
            "INSERT INTO graphile_worker.migrations (id) VALUES (1)",
        ];

        let tx = test_db.database.begin().await.unwrap();
        for stmt in initial_stmts {
            tx.execute(stmt, DbParams::new()).await.unwrap();
        }
        GRAPHILE_WORKER_MIGRATIONS[0]
            .execute(&tx, "graphile_worker")
            .await
            .expect("Failed to execute migration");
        tx.commit().await.unwrap();

        // Perform migration
        migrate(&test_db.database, "graphile_worker")
            .await
            .expect("Failed to migrate");

        // Assert migrations table exists and has relevant entries
        let migration_rows = test_db.get_migrations().await;
        assert!(
            migration_rows.len() >= 19,
            "There should be at least 19 migrations"
        );
        let migration_2 = migration_rows.iter().find(|m| m.id == 2).unwrap();
        assert!(!migration_2.breaking, "Migration 2 should not be breaking");
        let migration_11 = migration_rows.iter().find(|m| m.id == 11).unwrap();
        assert!(migration_11.breaking, "Migration 11 should be breaking");

        // Assert job schema files have been created
        test_db
            .add_job("assert_jobs_work", json!({}), Default::default())
            .await;

        let jobs_rows = test_db.get_jobs().await;
        assert_eq!(jobs_rows.len(), 1, "There should be one job");
        assert_eq!(
            jobs_rows[0].task_identifier, "assert_jobs_work",
            "The job should match 'assert_jobs_work'"
        );

        // Assert that re-migrating causes no issues
        for _ in 0..3 {
            migrate(&test_db.database, "graphile_worker")
                .await
                .expect("Failed to re-migrate");
        }

        let jobs_rows_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_rows_after.len(),
            1,
            "There should still be one job after re-migrating"
        );
        assert_eq!(
            jobs_rows_after[0].task_identifier, "assert_jobs_work",
            "The job should still match 'assert_jobs_work' after re-migrating"
        );
    })
    .await;
}
