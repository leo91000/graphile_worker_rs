use super::*;

#[tokio::test]
async fn recovery_hook_is_applied_to_dead_worker_sweep() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 3 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_hook_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create orphan lock");

        let calls = Arc::new(AtomicU32::new(0));
        let target_run_at = chrono::Utc::now() + chrono::Duration::seconds(5);
        let mut hooks = HookRegistry::new();
        let hook_calls = Arc::clone(&calls);
        hooks.on(JobRecovery, move |ctx| {
            let hook_calls = Arc::clone(&hook_calls);
            async move {
                assert_eq!(ctx.previous_worker_id, "dead_hook_worker");
                assert_eq!(ctx.reason, FailureReason::WorkerCrashed);
                hook_calls.fetch_add(1, Ordering::SeqCst);
                JobRecoveryResult::Reschedule {
                    run_at: target_run_at,
                    attempts: Some(3),
                }
            }
        });

        let sweep_utils =
            WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string())
                .with_hooks(Arc::new(hooks));
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
            })
            .await
            .expect("failed to sweep stale workers");

        assert_eq!(result.recovered_count, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 3);
        let run_at_delta_ms = (recovered.run_at - target_run_at).num_milliseconds().abs();
        assert!(
            run_at_delta_ms <= 1,
            "hook reschedule run_at should be preserved"
        );
    })
    .await;
}

#[tokio::test]
async fn recovery_hook_skip_leaves_job_locked() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 7 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_skip_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create skipped recovery lock");

        let mut hooks = HookRegistry::new();
        hooks.on(JobRecovery, |_ctx| async { JobRecoveryResult::Skip });

        let sweep_utils =
            WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string())
                .with_hooks(Arc::new(hooks));
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
            })
            .await
            .expect("failed to sweep skipped recovery lock");

        assert_eq!(result.worker_ids, vec!["dead_skip_worker".to_string()]);
        assert_eq!(result.recovered_count, 0);

        let jobs = test_db.get_jobs().await;
        let skipped = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert_eq!(skipped.locked_by.as_deref(), Some("dead_skip_worker"));
        assert_eq!(skipped.attempts, 1);
    })
    .await;
}

#[tokio::test]
async fn recovery_hook_fail_with_backoff_unlocks_with_retry_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 8 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_fail_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create fail-with-backoff recovery lock");

        let before_sweep = chrono::Utc::now();
        let mut hooks = HookRegistry::new();
        hooks.on(JobRecovery, |_ctx| async {
            JobRecoveryResult::FailWithBackoff
        });

        let sweep_utils =
            WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string())
                .with_hooks(Arc::new(hooks));
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
            })
            .await
            .expect("failed to sweep fail-with-backoff recovery lock");

        assert_eq!(result.worker_ids, vec!["dead_fail_worker".to_string()]);
        assert_eq!(result.recovered_count, 1);

        let jobs = test_db.get_jobs().await;
        let failed = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(failed.locked_by.is_none());
        assert_eq!(failed.attempts, 1);
        assert_eq!(failed.last_error.as_deref(), Some("WorkerCrashed"));
        assert!(
            failed.run_at >= before_sweep + chrono::Duration::seconds(1),
            "fail-with-backoff should apply normal retry delay"
        );
    })
    .await;
}
