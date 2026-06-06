use super::*;

#[tokio::test]
async fn local_queue_with_forbidden_flags_uses_direct_fetch() {
    with_test_db(|test_db| async move {
        FLAGGED_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(FlaggedJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        for i in 4..=6 {
            utils
                .add_job(
                    FlaggedJob { id: i },
                    JobSpec {
                        flags: Some(vec!["special".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job with flag");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(3)
                    .local_queue(LocalQueueConfig::builder().size(10).build())
                    .add_forbidden_flag("special")
                    .define_job::<FlaggedJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while FLAGGED_CALL_COUNT.get().await < 3 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Jobs without flag should have been executed by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        sleep(Duration::from_millis(500)).await;

        assert_eq!(
            FLAGGED_CALL_COUNT.get().await,
            3,
            "Only 3 jobs without the special flag should have been executed"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert_eq!(
            remaining_jobs.len(),
            3,
            "3 jobs with the special flag should remain in the queue"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_works_with_run_once() {
    with_test_db(|test_db| async move {
        RUN_ONCE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(RunOnceJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(3)
            .define_job::<RunOnceJob>()
            .init()
            .await
            .expect("Failed to create worker");

        worker.run_once().await.expect("Failed to run_once");

        assert_eq!(
            RUN_ONCE_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed with run_once"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert_eq!(
            remaining_jobs.len(),
            0,
            "No jobs should remain after run_once"
        );
    })
    .await;
}

#[tokio::test]
async fn local_queue_distributes_jobs_to_concurrent_workers() {
    with_test_db(|test_db| async move {
        CONCURRENT_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=10 {
            utils
                .add_job(ConcurrentDistributionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(5)
                    .local_queue(LocalQueueConfig::builder().size(20).build())
                    .define_job::<ConcurrentDistributionJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        while CONCURRENT_CALL_COUNT.get().await < 10 {
            if start.elapsed().as_secs() > 10 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    CONCURRENT_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(3),
            "With concurrency 5, 10 jobs at 100ms each should complete faster than sequential (10s), took {:?}",
            elapsed
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_transitions_modes_correctly() {
    with_test_db(|test_db| async move {
        MODE_TRANSITION_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .poll_interval(Duration::from_millis(100))
                .local_queue(LocalQueueConfig::builder().size(5).build())
                .listen_os_shutdown_signals(false)
                .define_job::<ModeTransitionJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(50)).await;

        for i in 1..=3 {
            utils
                .add_job(ModeTransitionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start_time = Instant::now();
        while MODE_TRANSITION_CALL_COUNT.get().await < 3 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("First batch should have been executed");
            }
            sleep(Duration::from_millis(50)).await;
        }

        sleep(Duration::from_millis(200)).await;

        for i in 4..=6 {
            utils
                .add_job(ModeTransitionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start_time = Instant::now();
        while MODE_TRANSITION_CALL_COUNT.get().await < 6 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Second batch should have been executed");
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            MODE_TRANSITION_CALL_COUNT.get().await,
            6,
            "All 6 jobs should have been executed across mode transitions"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}
