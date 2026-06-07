use super::*;

#[tokio::test]
async fn list_active_workers_reports_registered_metadata_and_stale_state() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let sql = formatdoc!(
            r#"
            INSERT INTO graphile_worker._private_workers
                (id, last_heartbeat_at, started_at, metadata)
            VALUES
                ('fresh_worker', now(), now() - interval '2 seconds', '{{"pid": 100}}'::jsonb),
                ('stale_worker', now() - interval '5 minutes', now() - interval '10 minutes', '{{"pid": 200}}'::jsonb)
            "#
        );
        safe_query(sql)
        .execute(&test_db.test_pool)
        .await
        .expect("failed to insert workers");

        let workers = utils
            .list_active_workers(Duration::from_secs(60))
            .await
            .expect("failed to list active workers");

        let fresh = workers
            .iter()
            .find(|worker| worker.worker_id == "fresh_worker")
            .expect("fresh worker should be listed");
        assert!(!fresh.is_stale);
        assert_eq!(fresh.metadata, Some(serde_json::json!({ "pid": 100 })));
        assert!(fresh.started_at <= fresh.last_heartbeat_at);

        let stale = workers
            .iter()
            .find(|worker| worker.worker_id == "stale_worker")
            .expect("stale worker should be listed");
        assert!(stale.is_stale);
        assert_eq!(stale.metadata, Some(serde_json::json!({ "pid": 200 })));
        assert!(stale.started_at < stale.last_heartbeat_at);
    })
    .await;
}
