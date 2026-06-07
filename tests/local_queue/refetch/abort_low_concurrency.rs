use super::*;

#[tokio::test]
async fn local_queue_refetch_delay_abort_with_low_concurrency() {
    with_test_db(|test_db| async move {
        REFETCH_ABORT_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_secs(30))
                .local_queue(
                    LocalQueueConfig::default()
                        .with_size(10)
                        .with_refetch_delay(
                            RefetchDelayConfig::default()
                                .with_duration(Duration::from_secs(30))
                                .with_threshold(0)
                                .with_max_abort_threshold(3),
                        ),
                )
                .listen_os_shutdown_signals(false)
                .define_job::<RefetchAbortJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(300)).await;

        for i in 1..=5 {
            utils
                .add_job(RefetchAbortJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
            sleep(Duration::from_millis(50)).await;
        }

        let start_time = Instant::now();
        while REFETCH_ABORT_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!(
                    "Jobs should have been executed (got {}). This would deadlock without the fix: \
                    with concurrency=2 and abort_threshold=3, handlers would block on oneshot \
                    channels before enough pulses could trigger the abort.",
                    REFETCH_ABORT_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            REFETCH_ABORT_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed after refetch delay abort"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}
