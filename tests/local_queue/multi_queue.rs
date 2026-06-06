use super::*;

#[tokio::test]
async fn local_queue_can_run_multiple_local_queues() {
    with_test_db(|test_db| async move {
        MULTI_LOCAL_QUEUE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=12 {
            utils
                .add_job(MultiLocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let init_count = Arc::new(AtomicU32::new(0));
        let plugin = LocalQueueInitCounterPlugin {
            counter: init_count.clone(),
        };

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(5)
                    .local_queue(LocalQueueConfig::default().with_size(3).with_queue_count(4))
                    .define_job::<MultiLocalQueueJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while MULTI_LOCAL_QUEUE_CALL_COUNT.get().await < 12 {
            if start_time.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!(
                    "All jobs should have been executed by now, got {}",
                    MULTI_LOCAL_QUEUE_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            init_count.load(Ordering::SeqCst),
            4,
            "A worker with queue_count=4 should start four local queues"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_count_is_limited_by_concurrency() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let init_count = Arc::new(AtomicU32::new(0));
        let plugin = LocalQueueInitCounterPlugin {
            counter: init_count.clone(),
        };

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .local_queue(LocalQueueConfig::default().with_size(3).with_queue_count(4))
                    .define_job::<MultiLocalQueueJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while init_count.load(Ordering::SeqCst) < 2 {
            if start_time.elapsed().as_secs() > 5 {
                worker_fut.abort();
                panic!(
                    "Expected two local queues to initialize, got {}",
                    init_count.load(Ordering::SeqCst)
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            init_count.load(Ordering::SeqCst),
            2,
            "queue_count should be capped at worker concurrency"
        );

        worker_fut.abort();
    })
    .await;
}
