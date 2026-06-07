use super::*;

#[tokio::test]
async fn test_complete_job_batch_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(10))
                .add_extension(counter.clone())
                .define_job::<SuccessJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..10 {
            utils
                .add_job(SuccessJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while counter.get() < 10 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Jobs should have completed by now, only {} completed",
                    counter.get()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(counter.get(), 10, "All 10 jobs should have completed");

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}
#[tokio::test]
async fn test_shutdown_flushes_pending_completions() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(100))
                .add_extension(counter.clone())
                .define_job::<SuccessJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..3 {
            utils
                .add_job(SuccessJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while counter.get() < 3 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!("Jobs should have run by now");
            }
            sleep(Duration::from_millis(50)).await;
        }

        worker.request_shutdown();
        let _ = worker_handle.await;

        let remaining_jobs: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM graphile_worker._private_jobs")
                .fetch_one(&test_db.test_pool)
                .await
                .expect("Failed to count jobs");

        assert_eq!(
            remaining_jobs.0, 0,
            "All jobs should have been completed and removed from the database"
        );
    })
    .await;
}
