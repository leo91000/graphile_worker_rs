use super::*;

#[tokio::test]
async fn test_local_queue_init_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .local_queue(LocalQueueConfig::builder().size(10).build())
            .define_job::<LocalQueueTestJob>()
            .add_plugin(plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let run_worker = worker.run();
        tokio::pin!(run_worker);

        let start_time = Instant::now();
        while counters.init.load(Ordering::SeqCst) == 0 {
            if start_time.elapsed().as_secs() > 5 {
                worker.request_shutdown();
                run_worker.await.expect("Failed to run worker");
                panic!(
                    "LocalQueue init hook should be called once, got {}",
                    counters.init.load(Ordering::SeqCst)
                );
            }

            tokio::select! {
                result = &mut run_worker => {
                    result.expect("Failed to run worker");
                    panic!("Worker stopped before LocalQueue init hook was called");
                }
                _ = sleep(Duration::from_millis(50)) => {}
            }
        }

        assert_eq!(
            counters.init.load(Ordering::SeqCst),
            1,
            "LocalQueue init hook should be called once"
        );

        worker.request_shutdown();
        run_worker.await.expect("Failed to run worker");
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_set_mode_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .local_queue(LocalQueueConfig::builder().size(10).build())
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let c = counters.clone();
        wait_for_condition(
            || c.set_mode.load(Ordering::SeqCst) >= 1,
            5,
            "LocalQueue set_mode hook should be called at least once (starting -> polling)",
        )
        .await;

        assert!(
            counters.set_mode.load(Ordering::SeqCst) >= 1,
            "LocalQueue set_mode hook should be called at least once (starting -> polling)"
        );

        let last_mode = *counters.last_mode_change.lock().unwrap();
        assert!(last_mode.is_some(), "Should have recorded a mode change");

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_get_jobs_complete_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        for i in 1..=5 {
            utils
                .add_job(LocalQueueTestJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .local_queue(LocalQueueConfig::builder().size(10).build())
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let c = counters.clone();
        wait_for_condition(
            || c.last_jobs_count.load(Ordering::SeqCst) >= 1,
            5,
            "Should have fetched at least one job",
        )
        .await;

        assert!(
            counters.get_jobs_complete.load(Ordering::SeqCst) >= 1,
            "get_jobs_complete hook should have been called"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_return_jobs_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        for i in 1..=10 {
            utils
                .add_job(SlowLocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::builder().size(20).build())
                .listen_os_shutdown_signals(false)
                .define_job::<SlowLocalQueueJob>()
                .add_plugin(plugin)
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        let c = counters.clone();
        wait_for_condition(
            || c.last_jobs_count.load(Ordering::SeqCst) >= 1,
            10,
            "Should have fetched at least one job into local queue",
        )
        .await;

        worker.request_shutdown();

        let c = counters.clone();
        wait_for_condition(
            || c.return_jobs.load(Ordering::SeqCst) >= 1,
            10,
            "return_jobs hook should be called on shutdown",
        )
        .await;

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_refetch_delay_hooks() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .local_queue(
                        LocalQueueConfig::builder()
                            .size(10)
                            .refetch_delay(
                                RefetchDelayConfig::builder()
                                    .duration(Duration::from_millis(50))
                                    .threshold(0)
                                    .build(),
                            )
                            .build(),
                    )
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        sleep(Duration::from_millis(500)).await;

        assert!(
            counters.refetch_delay_start.load(Ordering::SeqCst) >= 1,
            "refetch_delay_start hook should be called"
        );

        let c = counters.clone();
        wait_for_condition(
            || c.refetch_delay_expired.load(Ordering::SeqCst) >= 1,
            5,
            "refetch_delay_expired hook should be called",
        )
        .await;

        worker_fut.abort();
    })
    .await;
}
