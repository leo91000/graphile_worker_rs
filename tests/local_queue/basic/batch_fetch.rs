use super::*;

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

        wait_for_counter(
            &BATCH_CALL_COUNT,
            20,
            Duration::from_secs(10),
            Duration::from_millis(100),
            "All jobs should have been executed by now",
        )
        .await;

        assert_eq!(
            BATCH_CALL_COUNT.get().await,
            20,
            "All 20 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}
