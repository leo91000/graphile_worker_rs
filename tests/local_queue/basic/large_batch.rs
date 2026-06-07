use super::*;

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

        wait_for_counter(
            &LARGE_BATCH_CALL_COUNT,
            100,
            Duration::from_secs(30),
            Duration::from_millis(100),
            "All jobs should have been executed by now",
        )
        .await;

        assert_eq!(
            LARGE_BATCH_CALL_COUNT.get().await,
            100,
            "All 100 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}
