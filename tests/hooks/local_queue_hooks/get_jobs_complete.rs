use super::*;

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
