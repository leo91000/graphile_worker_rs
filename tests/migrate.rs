use graphile_worker_migrations::{
    migrate,
    sql::{
        m000001::M000001_MIGRATION, m000002::M000002_MIGRATION, m000003::M000003_MIGRATION,
        m000004::M000004_MIGRATION, m000005::M000005_MIGRATION, m000006::M000006_MIGRATION,
        m000007::M000007_MIGRATION, m000008::M000008_MIGRATION, m000009::M000009_MIGRATION,
        m000010::M000010_MIGRATION,
    },
    MigrateError,
};
use helpers::with_test_db;
use serde_json::json;
use sqlx::query;

mod helpers;

#[tokio::test]
async fn migration_install_schema_and_second_migration_does_not_harm() {
    with_test_db(|test_db| async move {
        query("drop schema if exists graphile_worker cascade")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        migrate(&test_db.test_pool, "graphile_worker")
            .await
            .expect("Failed to migrate");

        let migrations = test_db.get_migrations().await;

        assert_eq!(migrations.len(), 18);
        let m0 = &migrations[0];
        assert_eq!(m0.id, 1);

        test_db
            .add_job("assert_job_works", json!({}), Default::default())
            .await;

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!(job.task_identifier, "assert_job_works");

        for _ in 0..3 {
            migrate(&test_db.test_pool, "graphile_worker")
                .await
                .expect("Failed to migrate");
        }

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task_identifier, "assert_job_works");
    })
    .await;
}

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

        let mut tx = test_db.test_pool.begin().await.unwrap();
        for stmt in initial_stmts {
            query(stmt).execute(tx.as_mut()).await.unwrap();
        }
        M000001_MIGRATION
            .execute(&mut tx, "graphile_worker")
            .await
            .expect("Failed to execute migration");
        tx.commit().await.unwrap();

        // Perform migration
        migrate(&test_db.test_pool, "graphile_worker")
            .await
            .expect("Failed to migrate");

        // Assert migrations table exists and has relevant entries
        let migration_rows = test_db.get_migrations().await;
        assert!(
            migration_rows.len() >= 18,
            "There should be at least 18 migrations"
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
            migrate(&test_db.test_pool, "graphile_worker")
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

#[tokio::test]
async fn aborts_if_database_is_more_up_to_date_than_current_worker() {
    with_test_db(|test_db| async move {
        // Drop existing schema to ensure a clean state
        query("DROP SCHEMA IF EXISTS graphile_worker CASCADE;")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        // Perform initial migration
        migrate(&test_db.test_pool, "graphile_worker")
            .await
            .expect("Failed to perform initial migration");

        // Insert a more up-to-date migration to simulate a future schema version
        query("INSERT INTO graphile_worker.migrations (id, ts, breaking) VALUES (999999, '2023-10-19T10:31:00Z', true);")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        // Attempt to migrate again and expect it to fail due to version incompatibility
        let migration_result = migrate(&test_db.test_pool, "graphile_worker").await;

        assert!(
            matches!(
                migration_result,
                Err(MigrateError::IncompatbleRevision { latest_migration, latest_breaking_migration, highest_migration })
                if latest_migration == 999999 && latest_breaking_migration == 999999 && highest_migration == 18
            ),
            "Expected migration to abort due to database being more up to date than current worker"
        );
    })
    .await;
}

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
        let migrations = &[
            M000001_MIGRATION,
            M000002_MIGRATION,
            M000003_MIGRATION,
            M000004_MIGRATION,
            M000005_MIGRATION,
            M000006_MIGRATION,
            M000007_MIGRATION,
            M000008_MIGRATION,
            M000009_MIGRATION,
            M000010_MIGRATION,
        ];
        let mut tx = test_db.test_pool.begin().await.unwrap();
        for migration in migrations {
            migration.execute(&mut tx, "graphile_worker").await.unwrap();
            let sql = "insert into graphile_worker.migrations (id, breaking) values ($1, $2)";
            query(sql)
                .bind(migration.migration_number() as i64)
                .bind(migration.is_breaking())
                .execute(tx.as_mut())
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
        let migration_result = migrate(&test_db.test_pool, "graphile_worker").await;

        assert!(
            matches!(migration_result, Err(MigrateError::LockedJobInMigration11)),
            "Expected migration to fail due to locked jobs"
        );
    })
    .await;
}
