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
