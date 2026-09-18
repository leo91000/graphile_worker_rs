use super::*;

#[tokio::test]
async fn local_queue_returns_jobs_on_shutdown() {
    with_test_db(|test_db| async move {
        SHUTDOWN_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=10 {
            utils
                .add_job(ShutdownJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let initial_jobs = test_db.get_jobs().await;
        assert_eq!(initial_jobs.len(), 10, "Should have 10 jobs initially");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::builder().size(20).build())
                .listen_os_shutdown_signals(false)
                .shutdown_grace_period(Duration::from_millis(100))
                .define_job::<ShutdownJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        wait_for_jobs(
            &test_db,
            Duration::from_secs(5),
            Duration::from_millis(100),
            "Jobs should be locked by the worker before shutdown",
            |jobs| jobs.iter().filter(|j| j.locked_by.is_some()).count() >= 2,
        )
        .await;

        worker.request_shutdown();

        let start_time = Instant::now();
        while !worker_fut.is_finished() {
            if start_time.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!("Worker should have shut down by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        let remaining_jobs = wait_for_jobs(
            &test_db,
            Duration::from_secs(5),
            Duration::from_millis(100),
            "Most jobs should be returned to the queue",
            |jobs| jobs.iter().filter(|j| j.locked_by.is_none()).count() >= 8,
        )
        .await;
        let unlocked_jobs: Vec<_> = remaining_jobs
            .iter()
            .filter(|j| j.locked_by.is_none())
            .collect();

        assert!(
            unlocked_jobs.len() >= 8,
            "Most jobs should be returned to the queue (got {} unlocked out of {})",
            unlocked_jobs.len(),
            remaining_jobs.len()
        );

        assert_eq!(
            SHUTDOWN_CALL_COUNT.get().await,
            0,
            "No jobs should have completed (they take 10s each)"
        );
    })
    .await;
}

#[tokio::test]
async fn local_queue_returns_jobs_on_ttl_expiry() {
    with_test_db(|test_db| async move {
        TTL_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=10 {
            utils
                .add_job(TtlExpiryJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let initial_jobs = test_db.get_jobs().await;
        assert_eq!(initial_jobs.len(), 10, "Should have 10 jobs initially");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .local_queue(
                    LocalQueueConfig::builder()
                        .size(20)
                        .ttl(Duration::from_millis(500))
                        .build(),
                )
                .listen_os_shutdown_signals(false)
                .define_job::<TtlExpiryJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        let start_time = Instant::now();
        let locked_jobs_count = loop {
            let jobs_during_processing = test_db.get_jobs().await;
            let locked_jobs_count = jobs_during_processing
                .iter()
                .filter(|j| j.locked_by.is_some())
                .count();

            if locked_jobs_count > 0 || start_time.elapsed() > Duration::from_secs(5) {
                break locked_jobs_count;
            }

            sleep(Duration::from_millis(100)).await;
        };

        assert!(
            locked_jobs_count > 0,
            "At least one job should be locked by worker"
        );

        let start_time = Instant::now();
        let jobs_after_ttl = loop {
            let jobs = test_db.get_jobs().await;
            let unlocked_jobs_count = jobs.iter().filter(|j| j.locked_by.is_none()).count();

            if unlocked_jobs_count >= 8 || start_time.elapsed() > Duration::from_secs(5) {
                break jobs;
            }

            sleep(Duration::from_millis(100)).await;
        };
        let unlocked_jobs: Vec<_> = jobs_after_ttl
            .iter()
            .filter(|j| j.locked_by.is_none())
            .collect();

        assert!(
            unlocked_jobs.len() >= 8,
            "Most jobs should be returned after TTL expiry (got {} unlocked out of {})",
            unlocked_jobs.len(),
            jobs_after_ttl.len()
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_release_waits_for_run_loop() {
    with_test_db(|test_db| async move {
        RELEASE_WAITS_CALL_COUNT.reset().await;
        RELEASE_WAITS_COMPLETED.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(ReleaseWaitsJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::builder().size(10).build())
                .listen_os_shutdown_signals(false)
                .define_job::<ReleaseWaitsJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        let start = Instant::now();
        while RELEASE_WAITS_CALL_COUNT.get().await == 0 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!("At least one job should have started");
            }
            sleep(Duration::from_millis(50)).await;
        }

        worker.request_shutdown();

        let start = Instant::now();
        while !worker_fut.is_finished() {
            if start.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!("Worker should have finished shutdown by now");
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            worker_fut.is_finished(),
            "Worker future should be finished after shutdown"
        );
    })
    .await;
}

#[tokio::test]
async fn release_waits_for_in_flight_fetch_and_other_releasers() {
    use graphile_worker::local_queue::{LocalQueue, LocalQueueParams};
    use graphile_worker::sql::task_identifiers::get_tasks_details;
    use tokio::sync::Notify;

    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        db.worker_utils()
            .add_job(ShutdownJob { id: 1 }, JobSpec::default())
            .await
            .unwrap();
        let tasks = get_tasks_details(
            &db.database,
            "graphile_worker",
            vec![ShutdownJob::IDENTIFIER.into()],
        )
        .await
        .unwrap();
        let fetched = Arc::new(Notify::new());
        let allow_cache = Arc::new(Notify::new());
        let mut hooks = HookRegistry::default();
        let fetched_hook = fetched.clone();
        let allow_hook = allow_cache.clone();
        hooks.on(LocalQueueGetJobsComplete, move |_| {
            let fetched = fetched_hook.clone();
            let allow = allow_hook.clone();
            async move {
                fetched.notify_one();
                allow.notified().await;
            }
        });
        let (job_signal_sender, _rx) = graphile_worker_runtime::channel(1);
        let queue = LocalQueue::new(LocalQueueParams {
            config: LocalQueueConfig::builder().size(2).build(),
            database: db.database.clone(),
            schema: "graphile_worker".into(),
            worker_id: "release-race".into(),
            task_details: tasks.into(),
            poll_interval: Duration::from_secs(1),
            continuous: true,
            shutdown_signal: None,
            hooks: Arc::new(hooks),
            job_signal_sender,
            use_local_time: false,
        });
        tokio::time::timeout(Duration::from_secs(5), fetched.notified())
            .await
            .unwrap();
        let first = queue.release();
        let second = queue.release();
        futures::pin_mut!(first, second);
        assert!(
            futures::poll!(first.as_mut()).is_pending(),
            "release must wait for the fetched jobs to reach the cache"
        );
        assert!(
            futures::poll!(second.as_mut()).is_pending(),
            "concurrent release must wait for the same cleanup"
        );
        allow_cache.notify_one();
        tokio::time::timeout(Duration::from_secs(5), async {
            first.await.unwrap();
            second.await.unwrap();
        })
        .await
        .expect("release must finish after the fetch resumes");
        let jobs = db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert!(
            jobs[0].locked_by.is_none(),
            "the in-flight fetched job must be returned"
        );
        assert_eq!(jobs[0].attempts, 0);
        assert!(queue.get_job(&[]).await.is_none(), "Released is terminal");
    })
    .await;
}
