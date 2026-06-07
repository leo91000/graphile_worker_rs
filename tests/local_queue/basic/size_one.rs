use super::*;

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

        wait_for_counter(
            &SMALL_QUEUE_CALL_COUNT,
            5,
            Duration::from_secs(10),
            Duration::from_millis(100),
            "All jobs should have been executed by now",
        )
        .await;

        assert_eq!(
            SMALL_QUEUE_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed even with queue size 1"
        );

        worker_fut.abort();
    })
    .await;
}
