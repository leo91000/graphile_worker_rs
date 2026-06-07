use super::*;

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

        wait_for_counter(
            &CONCURRENT_CALL_COUNT,
            10,
            Duration::from_secs(10),
            Duration::from_millis(50),
            "All jobs should have been executed by now",
        )
        .await;

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
