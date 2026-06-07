use super::*;

#[tokio::test]
async fn queued_job_recovery_counts_jobs_and_unlocks_queue() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(
                LongJob { id: 9 },
                JobSpec::builder()
                    .queue_name("recovery_count_queue")
                    .build(),
            )
            .await
            .expect("failed to add queued job");

        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_jobs
                SET attempts = 1,
                    locked_by = $1::text,
                    locked_at = now() - interval '10 minutes'
                WHERE id = $2::bigint
            "#
        );
        safe_query(sql)
            .bind("dead_queued_worker")
            .bind(*job.id())
            .execute(&test_db.test_pool)
            .await
            .expect("failed to create queued job recovery lock");

        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_job_queues
                SET locked_by = $1::text,
                    locked_at = now() - interval '10 minutes'
                WHERE queue_name = 'recovery_count_queue'
            "#
        );
        safe_query(sql)
            .bind("dead_queued_worker")
            .execute(&test_db.test_pool)
            .await
            .expect("failed to create queued queue recovery lock");

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
            })
            .await
            .expect("failed to sweep queued recovery lock");

        assert_eq!(result.worker_ids, vec!["dead_queued_worker".to_string()]);
        assert_eq!(result.recovered_count, 1);

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");
        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 0);

        let queues = test_db.get_job_queues().await;
        let queue = queues
            .into_iter()
            .find(|queue| queue.queue_name == "recovery_count_queue")
            .expect("queue should exist");
        assert!(queue.locked_by.is_none());
        assert!(queue.locked_at.is_none());
    })
    .await;
}
