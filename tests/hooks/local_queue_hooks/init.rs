use super::*;

#[tokio::test]
async fn test_local_queue_init_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .local_queue(LocalQueueConfig::builder().size(10).build())
            .define_job::<LocalQueueTestJob>()
            .add_plugin(plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let run_worker = worker.run();
        tokio::pin!(run_worker);

        let start_time = Instant::now();
        while counters.init.load(Ordering::SeqCst) == 0 {
            if start_time.elapsed().as_secs() > 5 {
                worker.request_shutdown();
                run_worker.await.expect("Failed to run worker");
                panic!(
                    "LocalQueue init hook should be called once, got {}",
                    counters.init.load(Ordering::SeqCst)
                );
            }

            tokio::select! {
                result = &mut run_worker => {
                    result.expect("Failed to run worker");
                    panic!("Worker stopped before LocalQueue init hook was called");
                }
                _ = sleep(Duration::from_millis(50)) => {}
            }
        }

        assert_eq!(
            counters.init.load(Ordering::SeqCst),
            1,
            "LocalQueue init hook should be called once"
        );

        worker.request_shutdown();
        run_worker.await.expect("Failed to run worker");
    })
    .await;
}
