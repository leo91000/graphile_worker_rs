use super::*;

#[tokio::test]
async fn orphan_lock_is_recovered_after_threshold() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 2 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("graphile_worker_orphan")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create orphan lock");

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), graphile_worker::Schema::default());
        let before_sweep = chrono::Utc::now();
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(750)),
                dry_run: false,
            })
            .await
            .expect("failed to sweep orphan lock");

        assert!(result.worker_ids.contains(&"graphile_worker_orphan".to_string()));

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 0);
        assert!(
            recovered.run_at >= before_sweep + chrono::Duration::milliseconds(500),
            "subsecond recovery delay should not be truncated"
        );
    })
    .await;
}
#[tokio::test]
async fn recover_worker_sql_helpers_fetch_and_recover_locked_jobs() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let empty_recovered = recover_dead_worker_jobs(
            &test_db.database,
            &graphile_worker::Schema::default(),
            &[],
            Duration::from_secs(1),
        )
        .await
        .expect("failed to recover empty worker list");
        assert_eq!(empty_recovered, 0);

        let empty_locked = get_locked_jobs_for_recovery(
            &test_db.database,
            &graphile_worker::Schema::default(),
            &[],
        )
        .await
        .expect("failed to fetch empty locked job list");
        assert!(empty_locked.is_empty());

        let job = utils
            .add_job(LongJob { id: 11 }, JobSpec::default())
            .await
            .expect("failed to add recoverable job");
        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_jobs
                SET attempts = 1,
                    locked_by = 'recover_helper',
                    locked_at = now() - interval '10 minutes'
                WHERE id = $1
            "#
        );
        safe_query(sql)
            .bind(*job.id())
            .execute(&test_db.test_pool)
            .await
            .expect("failed to lock recoverable job");

        let worker_ids = vec!["recover_helper".to_string()];
        let locked_jobs = get_locked_jobs_for_recovery(
            &test_db.database,
            &graphile_worker::Schema::default(),
            &worker_ids,
        )
        .await
        .expect("failed to fetch locked jobs for recovery");
        assert_eq!(locked_jobs.len(), 1);
        assert_eq!(*locked_jobs[0].id(), *job.id());
        assert_eq!(
            locked_jobs[0].locked_by().as_deref(),
            Some("recover_helper")
        );

        let recovered = recover_dead_worker_jobs(
            &test_db.database,
            &graphile_worker::Schema::default(),
            &worker_ids,
            Duration::from_secs(2),
        )
        .await
        .expect("failed to recover locked jobs");
        assert_eq!(recovered, 1);

        let jobs = test_db.get_jobs().await;
        let recovered_job = jobs
            .into_iter()
            .find(|candidate| candidate.id == *job.id())
            .expect("job should exist");
        assert!(recovered_job.locked_by.is_none());
        assert_eq!(recovered_job.attempts, 0);
        assert_eq!(
            recovered_job.last_error.as_deref(),
            Some("Job recovered after worker interruption")
        );
    })
    .await;
}
