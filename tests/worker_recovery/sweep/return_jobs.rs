use super::*;

#[tokio::test]
async fn return_job_for_recovery_unlocks_queued_job_with_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(
                LongJob { id: 12 },
                JobSpec::builder()
                    .queue_name("direct_recovery_queue")
                    .build(),
            )
            .await
            .expect("failed to add queued recovery job");
        let queue_id = (*job.job_queue_id()).expect("queued job should have a queue id");

        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_jobs
                SET attempts = 1,
                    locked_by = 'queued_recovery_worker',
                    locked_at = now()
                WHERE id = $1
            "#
        );
        safe_query(sql)
            .bind(*job.id())
            .execute(&test_db.test_pool)
            .await
            .expect("failed to lock queued recovery job");
        let sql = formatdoc!(
            r#"
            UPDATE graphile_worker._private_job_queues
                SET locked_by = 'queued_recovery_worker',
                    locked_at = now()
                WHERE id = $1
            "#
        );
        safe_query(sql)
            .bind(queue_id)
            .execute(&test_db.test_pool)
            .await
            .expect("failed to lock queued recovery queue");

        let before_return = chrono::Utc::now();
        return_job_for_recovery(
            &test_db.database,
            &job,
            &graphile_worker::Schema::default(),
            "queued_recovery_worker",
            Some(Duration::from_secs(5)),
            Some("queued recovered"),
        )
        .await
        .expect("failed to return queued job for recovery");

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|candidate| candidate.id == *job.id())
            .expect("job should exist");
        assert!(recovered.locked_by.is_none());
        assert!(recovered.locked_at.is_none());
        assert_eq!(recovered.attempts, 0);
        assert_eq!(recovered.last_error.as_deref(), Some("queued recovered"));
        assert!(
            recovered.run_at >= before_return + chrono::Duration::seconds(4),
            "recovery delay should move queued job run_at"
        );

        let queues = test_db.get_job_queues().await;
        let queue = queues
            .into_iter()
            .find(|queue| queue.id == queue_id)
            .expect("queue should exist");
        assert!(queue.locked_by.is_none());
        assert!(queue.locked_at.is_none());
    })
    .await;
}
