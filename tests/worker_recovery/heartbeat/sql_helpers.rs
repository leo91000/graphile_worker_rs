use super::*;

#[tokio::test]
async fn heartbeat_sql_helpers_register_list_detect_and_deregister_workers() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        worker_heartbeat(
            &test_db.database,
            &graphile_worker::Schema::default(),
            "fresh_helper",
            Some(serde_json::json!({ "pid": 300 })),
        )
        .await
        .expect("failed to register fresh worker heartbeat");
        worker_heartbeat(
            &test_db.database,
            &graphile_worker::Schema::default(),
            "stale_helper",
            None,
        )
        .await
        .expect("failed to register stale worker heartbeat");

        let resilient_job = utils
            .add_job(
                LongJob { id: 10 },
                JobSpec::builder()
                    .flags(vec![INFRASTRUCTURE_RESILIENT_FLAG.to_string()])
                    .build(),
            )
            .await
            .expect("failed to add resilient job");
        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_jobs
                SET attempts = 1,
                    locked_by = 'fresh_helper',
                    locked_at = now()
                WHERE id = $1
            "#
        );
        safe_query(sql)
        .bind(*resilient_job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to lock resilient job");

        sqlx::query(
            "UPDATE graphile_worker._private_workers SET last_heartbeat_at = now() - interval '5 minutes' WHERE id = 'stale_helper'",
        )
        .execute(&test_db.test_pool)
        .await
        .expect("failed to age stale worker heartbeat");

        let stale_workers =
            list_stale_workers(&test_db.database, &graphile_worker::Schema::default(), Duration::from_secs(60))
                .await
                .expect("failed to list stale workers");
        assert_eq!(stale_workers, vec!["stale_helper".to_string()]);

        let orphan_workers =
            list_orphan_locked_workers(&test_db.database, &graphile_worker::Schema::default(), Duration::from_secs(60))
                .await
                .expect("failed to list orphan locked workers");
        assert!(orphan_workers.is_empty());

        let holds_no_configured_flags = worker_holds_resilient_locks(
            &test_db.database,
            &graphile_worker::Schema::default(),
            "fresh_helper",
            &[],
        )
        .await
        .expect("failed to check empty resilient flags");
        assert!(!holds_no_configured_flags);

        let holds_resilient_lock = worker_holds_resilient_locks(
            &test_db.database,
            &graphile_worker::Schema::default(),
            "fresh_helper",
            &[INFRASTRUCTURE_RESILIENT_FLAG.to_string()],
        )
        .await
        .expect("failed to check resilient locks");
        assert!(holds_resilient_lock);

        let heartbeat =
            get_worker_last_heartbeat(&test_db.database, &graphile_worker::Schema::default(), "fresh_helper")
                .await
                .expect("failed to fetch fresh worker heartbeat");
        assert!(heartbeat.is_some());

        let sweep_transaction = test_db.database.begin().await.expect("failed to begin sweep tx");
        assert!(
            try_acquire_sweep_lock(&sweep_transaction)
                .await
                .expect("failed to acquire sweep lock")
        );
        sweep_transaction
            .commit()
            .await
            .expect("failed to commit sweep tx");

        worker_deregister(&test_db.database, &graphile_worker::Schema::default(), "fresh_helper")
            .await
            .expect("failed to deregister fresh worker");
        let heartbeat =
            get_worker_last_heartbeat(&test_db.database, &graphile_worker::Schema::default(), "fresh_helper")
                .await
                .expect("failed to refetch fresh worker heartbeat");
        assert!(heartbeat.is_none());

        delete_stale_workers(
            &test_db.database,
            &graphile_worker::Schema::default(),
            &["stale_helper".to_string()],
        )
        .await
        .expect("failed to delete stale workers");
        let heartbeat =
            get_worker_last_heartbeat(&test_db.database, &graphile_worker::Schema::default(), "stale_helper")
                .await
                .expect("failed to refetch stale worker heartbeat");
        assert!(heartbeat.is_none());
    })
    .await;
}
