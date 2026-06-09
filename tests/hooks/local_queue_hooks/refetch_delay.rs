use super::*;

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

        let c = counters.clone();
        wait_for_condition(
            || c.refetch_delay_start.load(Ordering::SeqCst) >= 1,
            5,
            "refetch_delay_start hook should be called",
        )
        .await;

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
