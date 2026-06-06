use super::*;

#[tokio::test]
async fn local_queue_refetch_delay_triggers_when_below_threshold() {
    with_test_db(|test_db| async move {
        REFETCH_DELAY_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(RefetchDelayJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .poll_interval(Duration::from_secs(10))
                    .local_queue(
                        LocalQueueConfig::builder()
                            .size(10)
                            .refetch_delay(
                                RefetchDelayConfig::builder()
                                    .duration(Duration::from_millis(200))
                                    .threshold(5)
                                    .build(),
                            )
                            .build(),
                    )
                    .define_job::<RefetchDelayJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while REFETCH_DELAY_CALL_COUNT.get().await < 3 {
            if start_time.elapsed().as_secs() > 5 {
                panic!(
                    "Jobs should have been executed by now, got {}",
                    REFETCH_DELAY_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            REFETCH_DELAY_CALL_COUNT.get().await,
            3,
            "All 3 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_processes_jobs_with_refetch_delay() {
    with_test_db(|test_db| async move {
        REFETCH_WITH_JOBS_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(RefetchDelayWithJobsJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .poll_interval(Duration::from_millis(200))
                    .local_queue(
                        LocalQueueConfig::default()
                            .with_size(10)
                            .with_refetch_delay(
                                RefetchDelayConfig::default()
                                    .with_duration(Duration::from_millis(100))
                                    .with_threshold(3)
                                    .with_max_abort_threshold(10),
                            ),
                    )
                    .define_job::<RefetchDelayWithJobsJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while REFETCH_WITH_JOBS_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 10 {
                panic!(
                    "Jobs should have been executed, got {}",
                    REFETCH_WITH_JOBS_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            REFETCH_WITH_JOBS_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed with refetch delay configured"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_pulse_triggers_immediate_fetch() {
    with_test_db(|test_db| async move {
        PULSE_IMMEDIATE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .poll_interval(Duration::from_secs(30))
                .local_queue(LocalQueueConfig::builder().size(10).build())
                .listen_os_shutdown_signals(false)
                .define_job::<PulseImmediateFetchJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(500)).await;

        let start = Instant::now();
        utils
            .add_job(PulseImmediateFetchJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        while PULSE_IMMEDIATE_CALL_COUNT.get().await < 1 {
            if start.elapsed().as_secs() > 5 {
                panic!("Job should have been processed immediately via pulse, not after 30s poll");
            }
            sleep(Duration::from_millis(50)).await;
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(3),
            "Job should be processed quickly via pulse (took {:?}), not waiting for 30s poll_interval",
            elapsed
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_refetch_delay_abort_with_low_concurrency() {
    with_test_db(|test_db| async move {
        REFETCH_ABORT_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_secs(30))
                .local_queue(
                    LocalQueueConfig::default()
                        .with_size(10)
                        .with_refetch_delay(
                            RefetchDelayConfig::default()
                                .with_duration(Duration::from_secs(30))
                                .with_threshold(0)
                                .with_max_abort_threshold(3),
                        ),
                )
                .listen_os_shutdown_signals(false)
                .define_job::<RefetchAbortJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(300)).await;

        for i in 1..=5 {
            utils
                .add_job(RefetchAbortJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
            sleep(Duration::from_millis(50)).await;
        }

        let start_time = Instant::now();
        while REFETCH_ABORT_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!(
                    "Jobs should have been executed (got {}). This would deadlock without the fix: \
                    with concurrency=2 and abort_threshold=3, handlers would block on oneshot \
                    channels before enough pulses could trigger the abort.",
                    REFETCH_ABORT_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            REFETCH_ABORT_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed after refetch delay abort"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}
