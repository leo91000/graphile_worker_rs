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

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
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
            "graphile_worker",
            &[],
            Duration::from_secs(1),
        )
        .await
        .expect("failed to recover empty worker list");
        assert_eq!(empty_recovered, 0);

        let empty_locked = get_locked_jobs_for_recovery(&test_db.database, "graphile_worker", &[])
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
        let locked_jobs =
            get_locked_jobs_for_recovery(&test_db.database, "graphile_worker", &worker_ids)
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
            "graphile_worker",
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
            "graphile_worker",
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

#[tokio::test]
async fn background_sweeper_recovers_job_from_dead_worker() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 5 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let worker_a = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_secs(60))
                .sweep_threshold(Duration::from_millis(250))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker A"),
        );

        let worker_a_id = worker_a.worker_id().to_string();
        let worker_a_for_run = Arc::clone(&worker_a);
        let worker_a_fut = tokio::task::spawn(async move {
            let _ = worker_a_for_run.run().await;
        });

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.id == *job.id() && j.locked_by.as_deref() == Some(&worker_a_id))
            {
                break;
            }

            assert!(
                start.elapsed() <= Duration::from_secs(5),
                "worker A should lock the job before the test timeout"
            );
            sleep(Duration::from_millis(50)).await;
        }

        worker_a_fut.abort();
        let _ = worker_a_fut.await;

        let sweeper = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(100))
                .sweep_threshold(Duration::from_millis(250))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .init()
                .await
                .expect("failed to init sweeper worker"),
        );

        let sweeper_for_run = Arc::clone(&sweeper);
        let sweeper_fut = tokio::task::spawn(async move { sweeper_for_run.run().await });

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            let recovered = jobs
                .iter()
                .find(|j| j.id == *job.id())
                .is_some_and(|j| j.locked_by.is_none() && j.attempts == 0);
            if recovered {
                break;
            }

            if start.elapsed() > Duration::from_secs(8) {
                sweeper.request_shutdown();
                let _ = sweeper_fut.await;
                panic!("background sweeper should recover the dead worker job");
            }

            sleep(Duration::from_millis(100)).await;
        }

        sweeper.request_shutdown();
        sweeper_fut
            .await
            .expect("sweeper task should not panic")
            .expect("sweeper should shut down cleanly");
    })
    .await;
}

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
