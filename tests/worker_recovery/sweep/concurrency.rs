use super::*;

#[tokio::test]
async fn concurrent_sweeps_recover_locked_job_once() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 4 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_concurrent_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create orphan lock");

        let sweep_options = SweepStaleWorkersOptions {
            sweep_threshold: Some(Duration::from_secs(60)),
            recovery_delay: Some(Duration::from_millis(100)),
            dry_run: false,
        };
        let first_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let second_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());

        let (first, second) = tokio::join!(
            first_utils.sweep_stale_workers(sweep_options.clone()),
            second_utils.sweep_stale_workers(sweep_options)
        );

        let first = first.expect("first sweep should succeed");
        let second = second.expect("second sweep should succeed");
        assert_eq!(first.recovered_count + second.recovered_count, 1);

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
