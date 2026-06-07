use super::*;

#[tokio::test]
async fn test_retryable_failures_processed_individually() {
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

        utils
            .add_job(
                FailJob { id: 1 },
                JobSpec::builder().max_attempts(3).build(), // Will retry
            )
            .await
            .expect("Failed to add job");

        let start = Instant::now();
        while counter.get() < 1 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!("Job should have been attempted by now");
            }
            sleep(Duration::from_millis(50)).await;
        }

        sleep(Duration::from_millis(200)).await;

        let job: Option<(i64, i16, i16)> = sqlx::query_as(
            "SELECT id, attempts, max_attempts FROM graphile_worker._private_jobs LIMIT 1",
        )
        .fetch_optional(&test_db.test_pool)
        .await
        .expect("Failed to query job");

        assert!(job.is_some(), "Job should still exist for retry");
        let (_, attempts, max_attempts) = job.unwrap();
        assert_eq!(attempts, 1, "Job should have 1 attempt");
        assert_eq!(max_attempts, 3, "Job should have max_attempts=3");

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}
