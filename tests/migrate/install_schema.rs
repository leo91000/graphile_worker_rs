use super::*;

async fn execute_statements(test_db: &helpers::TestDatabase, statements: &[&'static str]) {
    for statement in statements {
        query(*statement)
            .execute(&test_db.test_pool)
            .await
            .expect("test setup statement should execute");
    }
}

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

#[tokio::test]
async fn concurrent_migrations_on_fresh_database_succeed() {
    with_test_db(|test_db| async move {
        query("drop schema if exists graphile_worker cascade")
            .execute(&test_db.test_pool)
            .await
            .unwrap();

        let database_1 = test_db.database.clone();
        let database_2 = test_db.database.clone();
        let database_3 = test_db.database.clone();
        let database_4 = test_db.database.clone();

        let (result_1, result_2, result_3, result_4) = tokio::join!(
            migrate(&database_1, "graphile_worker"),
            migrate(&database_2, "graphile_worker"),
            migrate(&database_3, "graphile_worker"),
            migrate(&database_4, "graphile_worker"),
        );

        result_1.expect("first concurrent migration should succeed");
        result_2.expect("second concurrent migration should succeed");
        result_3.expect("third concurrent migration should succeed");
        result_4.expect("fourth concurrent migration should succeed");

        let migrations = test_db.get_migrations().await;
        assert_eq!(migrations.len(), 20);
        assert_eq!(migrations.first().map(|migration| migration.id), Some(1));
        assert_eq!(migrations.last().map(|migration| migration.id), Some(20));
    })
    .await;
}

#[tokio::test]
async fn concurrent_migration_insert_clash_continues() {
    with_test_db(|test_db| async move {
        query("drop schema if exists graphile_worker cascade")
            .execute(&test_db.test_pool)
            .await
            .unwrap();
        execute_statements(
            &test_db,
            &[
                "create schema graphile_worker",
                r#"
                create table graphile_worker.migrations (
                id int primary key,
                ts timestamptz default now() not null,
                breaking boolean not null default false
                )
                "#,
                r#"
                create function graphile_worker.raise_migration_insert_clash()
                returns trigger
                language plpgsql
                as $$
                begin
                    raise unique_violation using message = 'concurrent migration insert';
                end;
                $$
                "#,
                r#"
                create trigger raise_migration_insert_clash
                before insert on graphile_worker.migrations
                for each row execute function graphile_worker.raise_migration_insert_clash()
                "#,
            ],
        )
        .await;

        migrate(&test_db.database, "graphile_worker")
            .await
            .expect("insert clash should be treated as concurrent migration");
    })
    .await;
}

#[tokio::test]
async fn migration_insert_non_clash_error_is_returned() {
    with_test_db(|test_db| async move {
        query("drop schema if exists graphile_worker cascade")
            .execute(&test_db.test_pool)
            .await
            .unwrap();
        execute_statements(
            &test_db,
            &[
                "create schema graphile_worker",
                r#"
                create table graphile_worker.migrations (
                id int primary key,
                ts timestamptz default now() not null,
                breaking boolean not null default false
                )
                "#,
                r#"
                create function graphile_worker.raise_migration_insert_error()
                returns trigger
                language plpgsql
                as $$
                begin
                    raise exception 'unexpected migration insert failure' using errcode = '22012';
                end;
                $$
                "#,
                r#"
                create trigger raise_migration_insert_error
                before insert on graphile_worker.migrations
                for each row execute function graphile_worker.raise_migration_insert_error()
                "#,
            ],
        )
        .await;

        let error = migrate(&test_db.database, "graphile_worker")
            .await
            .expect_err("non-clash insert errors should be returned");

        match error {
            MigrateError::SqlError(error) => assert_eq!(error.code(), Some("22012")),
            error => panic!("expected SQL error, got {error:?}"),
        }
    })
    .await;
}
