use super::*;

#[tokio::test]
async fn resilient_job_uses_extended_sweep_threshold() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let worker_id = "resilient_worker";
        let job = utils
            .add_job(
                LongJob { id: 6 },
                JobSpec::builder()
                    .flags(vec![INFRASTRUCTURE_RESILIENT_FLAG.to_string()])
                    .build(),
            )
            .await
            .expect("failed to add job");

        sqlx::query("SELECT graphile_worker.worker_heartbeat($1::text)")
            .bind(worker_id)
            .execute(&test_db.test_pool)
            .await
            .expect("failed to register resilient worker heartbeat");

        sqlx::query(
            "UPDATE graphile_worker._private_workers SET last_heartbeat_at = now() - interval '2 minutes' WHERE id = $1",
        )
        .bind(worker_id)
        .execute(&test_db.test_pool)
        .await
        .expect("failed to age resilient worker heartbeat");

        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_jobs
                SET attempts = 1,
                    locked_by = $1::text,
                    locked_at = now() - interval '2 minutes'
                WHERE id = $2::bigint
            "#
        );
        safe_query(sql)
        .bind(worker_id)
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create resilient stale worker lock");

        let config = WorkerRecoveryConfig::default()
            .sweep_threshold(Duration::from_secs(60))
            .resilient_sweep_threshold_multiplier(3);
        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());

        let result = sweep_utils
            .sweep_stale_workers_with_config(
                &config,
                SweepStaleWorkersOptions {
                    recovery_delay: Some(Duration::from_millis(100)),
                    dry_run: false,
                    ..Default::default()
                },
            )
            .await
            .expect("failed to sweep resilient worker before extended threshold");

        assert!(result.worker_ids.is_empty());
        let jobs = test_db.get_jobs().await;
        let still_locked = jobs
            .iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");
        assert_eq!(still_locked.locked_by.as_deref(), Some(worker_id));

        sqlx::query(
            "UPDATE graphile_worker._private_workers SET last_heartbeat_at = now() - interval '4 minutes' WHERE id = $1",
        )
        .bind(worker_id)
        .execute(&test_db.test_pool)
        .await
        .expect("failed to age resilient worker heartbeat");

        let result = sweep_utils
            .sweep_stale_workers_with_config(
                &config,
                SweepStaleWorkersOptions {
                    recovery_delay: Some(Duration::from_millis(100)),
                    dry_run: false,
                    ..Default::default()
                },
            )
            .await
            .expect("failed to sweep resilient worker after extended threshold");

        assert_eq!(result.worker_ids, vec![worker_id.to_string()]);
        assert_eq!(result.recovered_count, 1);

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 0);
    })
    .await;
}
