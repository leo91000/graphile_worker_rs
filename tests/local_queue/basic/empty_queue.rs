use super::*;

#[tokio::test]
async fn local_queue_handles_empty_queue_gracefully() {
    with_test_db(|test_db| async move {
        EMPTY_QUEUE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let get_jobs_complete_count = Arc::new(AtomicU32::new(0));
        let plugin = LocalQueueGetJobsCompleteCounterPlugin {
            counter: get_jobs_complete_count.clone(),
        };

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(100))
                .local_queue(LocalQueueConfig::builder().size(10).build())
                .listen_os_shutdown_signals(false)
                .define_job::<EmptyQueueJob>()
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
            &get_jobs_complete_count,
            1,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "Local queue should have completed an empty fetch",
        )
        .await;

        assert_eq!(
            EMPTY_QUEUE_CALL_COUNT.get().await,
            0,
            "No jobs should have been executed on empty queue"
        );

        utils
            .add_job(EmptyQueueJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        wait_for_counter(
            &EMPTY_QUEUE_CALL_COUNT,
            1,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "Job should have been executed",
        )
        .await;

        assert_eq!(
            EMPTY_QUEUE_CALL_COUNT.get().await,
            1,
            "Job should have been executed after being added to empty queue"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}
