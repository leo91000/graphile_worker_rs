use super::*;

#[tokio::test]
async fn test_local_queue_return_jobs_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        for i in 1..=10 {
            utils
                .add_job(SlowLocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::builder().size(20).build())
                .listen_os_shutdown_signals(false)
                .define_job::<SlowLocalQueueJob>()
                .add_plugin(plugin)
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        let c = counters.clone();
        wait_for_condition(
            || c.last_jobs_count.load(Ordering::SeqCst) >= 1,
            10,
            "Should have fetched at least one job into local queue",
        )
        .await;

        worker.request_shutdown();

        let c = counters.clone();
        wait_for_condition(
            || c.return_jobs.load(Ordering::SeqCst) >= 1,
            10,
            "return_jobs hook should be called on shutdown",
        )
        .await;

        worker_fut.abort();
    })
    .await;
}
