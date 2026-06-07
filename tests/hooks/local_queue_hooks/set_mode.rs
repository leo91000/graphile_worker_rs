use super::*;

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
