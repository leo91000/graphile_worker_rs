use super::*;

#[tokio::test]
async fn test_fail_job_batch_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .fail_job_batch_delay(Duration::from_millis(10))
                .add_extension(counter.clone())
                .define_job::<FailJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..5 {
            utils
                .add_job(
                    FailJob { id: i },
                    JobSpec::builder().max_attempts(1).build(), // No retries
                )
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while counter.get() < 5 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Jobs should have been attempted by now, only {} attempted",
                    counter.get()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(counter.get(), 5, "All 5 jobs should have been attempted");

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}
