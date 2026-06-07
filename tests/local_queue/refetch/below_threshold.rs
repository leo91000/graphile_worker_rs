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

        wait_for_counter(
            &REFETCH_DELAY_CALL_COUNT,
            3,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "Jobs should have been executed by now",
        )
        .await;

        assert_eq!(
            REFETCH_DELAY_CALL_COUNT.get().await,
            3,
            "All 3 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}
