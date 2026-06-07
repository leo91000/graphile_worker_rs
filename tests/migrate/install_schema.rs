use super::*;

#[tokio::test]
async fn migration_install_schema_and_second_migration_does_not_harm() {
    with_test_db(|test_db| async move {
        query("drop schema if exists graphile_worker cascade")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        migrate(&test_db.database, "graphile_worker")
            .await
            .expect("Failed to migrate");

        let migrations = test_db.get_migrations().await;

        assert_eq!(migrations.len(), 20);
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
            migrate(&test_db.database, "graphile_worker")
                .await
                .expect("Failed to migrate");
        }

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task_identifier, "assert_job_works");
    })
    .await;
}
