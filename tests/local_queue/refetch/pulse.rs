use super::*;

#[tokio::test]
async fn local_queue_pulse_triggers_immediate_fetch() {
    with_test_db(|test_db| async move {
        PULSE_IMMEDIATE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let init_count = Arc::new(AtomicU32::new(0));
        let plugin = LocalQueueInitCounterPlugin {
            counter: init_count.clone(),
        };

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .poll_interval(Duration::from_secs(30))
                .local_queue(LocalQueueConfig::builder().size(10).build())
                .listen_os_shutdown_signals(false)
                .define_job::<PulseImmediateFetchJob>()
                .add_plugin(plugin)
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        wait_for_atomic_counter(
            &init_count,
            1,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "Local queue should have initialized before adding the job",
        )
        .await;

        utils
            .add_job(PulseImmediateFetchJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        wait_for_counter(
            &PULSE_IMMEDIATE_CALL_COUNT,
            1,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "Job should have been processed immediately via pulse, not after 30s poll",
        )
        .await;

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}
