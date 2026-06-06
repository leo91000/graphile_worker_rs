use super::*;

#[tokio::test]
async fn stale_worker_heartbeat_recovers_locked_jobs() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        utils
            .add_job(LongJob { id: 1 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(200))
                .sweep_threshold(Duration::from_millis(300))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );

        let worker_id = worker.worker_id().to_string();
        let worker_for_run = Arc::clone(&worker);
        let worker_fut = tokio::task::spawn(async move {
            let _ = worker_for_run.run().await;
        });

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.locked_by.as_deref() == Some(worker_id.as_str()))
            {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        worker_fut.abort();
        let _ = worker_fut.await;

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let sweep_start = Instant::now();
        let mut recovered = false;
        while sweep_start.elapsed() < Duration::from_secs(10) {
            let result = sweep_utils
                .sweep_stale_workers(SweepStaleWorkersOptions {
                    sweep_threshold: Some(Duration::from_millis(300)),
                    recovery_delay: Some(Duration::from_millis(100)),
                    dry_run: false,
                })
                .await
                .expect("failed to sweep stale workers");

            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.locked_by.is_none() && j.attempts == 0)
            {
                recovered =
                    !result.worker_ids.is_empty() || jobs.iter().any(|j| j.locked_by.is_none());
                if recovered {
                    break;
                }
            }

            sleep(Duration::from_millis(250)).await;
        }

        assert!(recovered, "job should be recovered from stale worker");
    })
    .await;
}

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

#[tokio::test]
async fn heartbeat_sql_helpers_register_list_detect_and_deregister_workers() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        worker_heartbeat(
            &test_db.database,
            "graphile_worker",
            "fresh_helper",
            Some(serde_json::json!({ "pid": 300 })),
        )
        .await
        .expect("failed to register fresh worker heartbeat");
        worker_heartbeat(
            &test_db.database,
            "graphile_worker",
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
            list_stale_workers(&test_db.database, "graphile_worker", Duration::from_secs(60))
                .await
                .expect("failed to list stale workers");
        assert_eq!(stale_workers, vec!["stale_helper".to_string()]);

        let orphan_workers =
            list_orphan_locked_workers(&test_db.database, "graphile_worker", Duration::from_secs(60))
                .await
                .expect("failed to list orphan locked workers");
        assert!(orphan_workers.is_empty());

        let holds_no_configured_flags = worker_holds_resilient_locks(
            &test_db.database,
            "graphile_worker",
            "fresh_helper",
            &[],
        )
        .await
        .expect("failed to check empty resilient flags");
        assert!(!holds_no_configured_flags);

        let holds_resilient_lock = worker_holds_resilient_locks(
            &test_db.database,
            "graphile_worker",
            "fresh_helper",
            &[INFRASTRUCTURE_RESILIENT_FLAG.to_string()],
        )
        .await
        .expect("failed to check resilient locks");
        assert!(holds_resilient_lock);

        let heartbeat =
            get_worker_last_heartbeat(&test_db.database, "graphile_worker", "fresh_helper")
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

        worker_deregister(&test_db.database, "graphile_worker", "fresh_helper")
            .await
            .expect("failed to deregister fresh worker");
        let heartbeat =
            get_worker_last_heartbeat(&test_db.database, "graphile_worker", "fresh_helper")
                .await
                .expect("failed to refetch fresh worker heartbeat");
        assert!(heartbeat.is_none());

        delete_stale_workers(
            &test_db.database,
            "graphile_worker",
            &["stale_helper".to_string()],
        )
        .await
        .expect("failed to delete stale workers");
        let heartbeat =
            get_worker_last_heartbeat(&test_db.database, "graphile_worker", "stale_helper")
                .await
                .expect("failed to refetch stale worker heartbeat");
        assert!(heartbeat.is_none());
    })
    .await;
}
