use super::*;

/// Builds either historical revision-20 schema before exercising an upgrade.
async fn install_revision_20(db: &helpers::TestDatabase, upstream: bool) {
    db.database
        .execute("create schema graphile_worker", DbParams::new())
        .await
        .unwrap();
    db.database.execute("create table graphile_worker.migrations(id int primary key, ts timestamptz not null default now(), breaking boolean not null default false)", DbParams::new()).await.unwrap();
    let tx = db.database.begin().await.unwrap();
    for migration in &GRAPHILE_WORKER_MIGRATIONS[..if upstream { 18 } else { 20 }] {
        migration.execute(&tx, "graphile_worker").await.unwrap();
        tx.execute(
            "insert into graphile_worker.migrations(id, breaking) values ($1, $2)",
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
    if upstream {
        // Official v0.18.0 migration 20 (4cda192), on the shared revision-18
        // schema. Upstream revision 19 is only a breaking-change marker.
        let sql = include_str!("../fixtures/upstream-0.18.0-000020.sql")
            .replace(":GRAPHILE_WORKER_SCHEMA", "graphile_worker");
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&db.test_pool)
            .await
            .unwrap();
        db.database.execute("insert into graphile_worker.migrations(id, breaking) values (19, true), (20, false)", DbParams::new()).await.unwrap();
    }
}

/// Checks existing jobs, locks and recovery data survive either upgrade history.
async fn check_upgrade(db: helpers::TestDatabase, upstream: bool) {
    install_revision_20(&db, upstream).await;
    db.add_job(
        "preserved",
        json!([1, 2]),
        graphile_worker::JobSpec {
            queue_name: Some("serial".into()),
            job_key: Some("key".into()),
            ..Default::default()
        },
    )
    .await;
    query("update graphile_worker._private_jobs set locked_by = 'old-worker', locked_at = now(), attempts = 1").execute(&db.test_pool).await.unwrap();
    query("update graphile_worker._private_job_queues set locked_by = 'old-worker', locked_at = now()").execute(&db.test_pool).await.unwrap();
    let before = db.get_jobs().await;
    let ledger: Vec<(i32, chrono::DateTime<chrono::Utc>, bool)> =
        sqlx::query_as("select id, ts, breaking from graphile_worker.migrations order by id")
            .fetch_all(&db.test_pool)
            .await
            .unwrap();
    if !upstream {
        query("select graphile_worker.worker_heartbeat('old-worker', '{\"keep\":true}')")
            .execute(&db.test_pool)
            .await
            .unwrap();
    }
    migrate(&db.database, "graphile_worker").await.unwrap();
    assert_eq!(db.get_jobs().await, before);
    let after: Vec<(i32, chrono::DateTime<chrono::Utc>, bool)> = sqlx::query_as(
        "select id, ts, breaking from graphile_worker.migrations where id <= 20 order by id",
    )
    .fetch_all(&db.test_pool)
    .await
    .unwrap();
    assert_eq!(
        after, ledger,
        "applied migration history must remain unchanged"
    );
    if !upstream {
        let metadata: serde_json::Value = sqlx::query_scalar(
            "select metadata from graphile_worker._private_workers where id = 'old-worker'",
        )
        .fetch_one(&db.test_pool)
        .await
        .unwrap();
        assert_eq!(metadata, json!({"keep": true}));
    }
    // Recovery functions must exist even when upgrading from Node's revision 20.
    query("select graphile_worker.worker_heartbeat('new-worker')")
        .execute(&db.test_pool)
        .await
        .unwrap();
    migrate(&db.database, "graphile_worker").await.unwrap();
    assert_eq!(db.get_jobs().await, before);
    db.add_job(
        "preserved",
        json!([3]),
        graphile_worker::JobSpec {
            job_key: Some("key".into()),
            ..Default::default()
        },
    )
    .await;
    let jobs = db.get_jobs().await;
    assert_eq!(
        jobs.len(),
        2,
        "locked keyed job must retain the Rust replacement behavior"
    );
    assert_eq!(jobs[0].key, None);
    assert_eq!(jobs[0].payload, json!([1, 2]));
    assert_eq!(jobs[1].payload, json!([3]));
}

/// Covers upgrade from Rust's revision 20 with its existing recovery objects.
#[tokio::test]
async fn identity_fix_upgrades_rust_revision_20_without_losing_jobs() {
    with_test_db(|db| check_upgrade(db, false)).await;
}

/// Covers upstream's distinct revision 20 and installation of Rust recovery objects.
#[tokio::test]
async fn identity_fix_upgrades_upstream_revision_20_with_recovery_support() {
    with_test_db(|db| check_upgrade(db, true)).await;
}

/// Proves known task and queue names remain usable when identity sequences are full.
#[tokio::test]
async fn identity_fix_allows_existing_names_after_sequence_exhaustion() {
    with_test_db(|db| async move {
        install_revision_20(&db, false).await;
        db.add_job("existing", json!({}), graphile_worker::JobSpec {
            queue_name: Some("existing".into()), ..Default::default()
        }).await;
        query("select setval(pg_get_serial_sequence('graphile_worker._private_tasks', 'id'), 2147483647)").execute(&db.test_pool).await.unwrap();
        query("select setval(pg_get_serial_sequence('graphile_worker._private_job_queues', 'id'), 2147483647)").execute(&db.test_pool).await.unwrap();
        let before = query("select graphile_worker.add_job('existing', queue_name => 'existing')")
            .execute(&db.test_pool).await.unwrap_err();
        assert_eq!(before.as_database_error().unwrap().code().as_deref(), Some("2200H"));
        migrate(&db.database, "graphile_worker").await.unwrap();
        for mode in ["replace", "preserve_run_at", "unsafe_dedupe"] {
            query("select graphile_worker.add_job('existing', queue_name => 'existing', job_key => 'key', job_key_mode => $1)")
                .bind(mode).execute(&db.test_pool).await.unwrap();
        }
        graphile_worker::sql::task_identifiers::get_tasks_details(&db.database, "graphile_worker", vec!["existing".into()]).await.unwrap();
        assert_eq!(db.get_jobs().await.len(), 2);
    }).await;
}

/// Ensures migration cannot misclassify historical ordinary final attempts as retired.
#[tokio::test]
async fn retirement_marker_upgrade_preserves_existing_final_attempts() {
    with_test_db(|db| async move {
        install_revision_20(&db, false).await;
        let tx = db.database.begin().await.unwrap();
        GRAPHILE_WORKER_MIGRATIONS[20].execute(&tx, "graphile_worker").await.unwrap();
        tx.execute("insert into graphile_worker.migrations(id, breaking) values (21, false)", DbParams::new()).await.unwrap();
        tx.commit().await.unwrap();
        db.add_job("existing", json!({}), graphile_worker::JobSpec { max_attempts: Some(1), ..Default::default() }).await;
        query("update graphile_worker._private_jobs set locked_by = 'owner', locked_at = now(), attempts = 1").execute(&db.test_pool).await.unwrap();
        let before = db.get_jobs().await;
        migrate(&db.database, "graphile_worker").await.unwrap();
        assert_eq!(db.get_jobs().await, before);
        let marker: bool = sqlx::query_scalar("select exists (select 1 from graphile_worker._private_job_retirements)").fetch_one(&db.test_pool).await.unwrap();
        assert!(!marker, "existing final attempts must not be guessed to be replacements");
        query("select graphile_worker.recover_dead_worker_jobs(array['owner'])").execute(&db.test_pool).await.unwrap();
        let jobs = db.get_jobs().await;
        assert_eq!(jobs[0].attempts, 0);
        assert!(jobs[0].locked_by.is_none());
    }).await;
}
