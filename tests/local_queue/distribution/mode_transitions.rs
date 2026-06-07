use super::*;

#[tokio::test]
async fn local_queue_transitions_modes_correctly() {
    with_test_db(|test_db| async move {
        MODE_TRANSITION_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .poll_interval(Duration::from_millis(100))
                .local_queue(LocalQueueConfig::builder().size(5).build())
                .listen_os_shutdown_signals(false)
                .define_job::<ModeTransitionJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(50)).await;

        for i in 1..=3 {
            utils
                .add_job(ModeTransitionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        wait_for_counter(
            &MODE_TRANSITION_CALL_COUNT,
            3,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "First batch should have been executed",
        )
        .await;

        sleep(Duration::from_millis(200)).await;

        for i in 4..=6 {
            utils
                .add_job(ModeTransitionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        wait_for_counter(
            &MODE_TRANSITION_CALL_COUNT,
            6,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "Second batch should have been executed",
        )
        .await;

        assert_eq!(
            MODE_TRANSITION_CALL_COUNT.get().await,
            6,
            "All 6 jobs should have been executed across mode transitions"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}
