use super::*;

#[tokio::test]
async fn local_queue_processes_jobs_correctly() {
    with_test_db(|test_db| async move {
        LOCAL_QUEUE_JOB_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(3)
                    .local_queue(LocalQueueConfig::builder().size(10).build())
                    .define_job::<LocalQueueJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        for i in 1..=5 {
            utils
                .add_job(LocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");

            let start_time = Instant::now();
            while LOCAL_QUEUE_JOB_CALL_COUNT.get().await < i {
                if start_time.elapsed().as_secs() > 5 {
                    panic!("Job should have been executed by now");
                }
                sleep(Duration::from_millis(100)).await;
            }

            assert_eq!(
                LOCAL_QUEUE_JOB_CALL_COUNT.get().await,
                i,
                "Job should have been executed {} times",
                i
            );
        }

        sleep(Duration::from_secs(1)).await;
        assert_eq!(
            LOCAL_QUEUE_JOB_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_batch_fetches_jobs() {
    with_test_db(|test_db| async move {
        BATCH_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=20 {
            utils
                .add_job(BatchJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(5)
                    .local_queue(LocalQueueConfig::builder().size(50).build())
                    .define_job::<BatchJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while BATCH_CALL_COUNT.get().await < 20 {
            if start_time.elapsed().as_secs() > 10 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    BATCH_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            BATCH_CALL_COUNT.get().await,
            20,
            "All 20 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_with_size_one() {
    with_test_db(|test_db| async move {
        SMALL_QUEUE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(SmallQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(1)
                    .local_queue(LocalQueueConfig::builder().size(1).build())
                    .define_job::<SmallQueueJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while SMALL_QUEUE_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 10 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    SMALL_QUEUE_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            SMALL_QUEUE_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed even with queue size 1"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_handles_empty_queue_gracefully() {
    with_test_db(|test_db| async move {
        EMPTY_QUEUE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(100))
                .local_queue(LocalQueueConfig::builder().size(10).build())
                .listen_os_shutdown_signals(false)
                .define_job::<EmptyQueueJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(300)).await;

        assert_eq!(
            EMPTY_QUEUE_CALL_COUNT.get().await,
            0,
            "No jobs should have been executed on empty queue"
        );

        utils
            .add_job(EmptyQueueJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        let start_time = Instant::now();
        while EMPTY_QUEUE_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job should have been executed");
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            EMPTY_QUEUE_CALL_COUNT.get().await,
            1,
            "Job should have been executed after being added to empty queue"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn local_queue_handles_large_batch() {
    with_test_db(|test_db| async move {
        LARGE_BATCH_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=100 {
            utils
                .add_job(LargeBatchJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(10)
                    .local_queue(LocalQueueConfig::builder().size(50).build())
                    .define_job::<LargeBatchJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while LARGE_BATCH_CALL_COUNT.get().await < 100 {
            if start_time.elapsed().as_secs() > 30 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    LARGE_BATCH_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            LARGE_BATCH_CALL_COUNT.get().await,
            100,
            "All 100 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}
