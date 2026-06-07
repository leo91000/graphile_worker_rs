use super::*;

#[tokio::test]
async fn test_permanent_failure_unlocks_job_and_queue() {
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
                JobSpec::builder()
                    .max_attempts(1)
                    .queue_name("test_queue")
                    .build(),
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

        let start = Instant::now();
        let (job, queue) = loop {
            let job: Option<(i64, Option<String>, Option<chrono::DateTime<chrono::Utc>>)> =
                sqlx::query_as(
                    "SELECT id, locked_by, locked_at FROM graphile_worker._private_jobs LIMIT 1",
                )
                .fetch_optional(&test_db.test_pool)
                .await
                .expect("Failed to query job");

            let queue: Option<(i32, Option<String>, Option<chrono::DateTime<chrono::Utc>>)> =
                sqlx::query_as(
                    "SELECT id, locked_by, locked_at FROM graphile_worker._private_job_queues WHERE queue_name = 'test_queue' LIMIT 1",
                )
                .fetch_optional(&test_db.test_pool)
                .await
                .expect("Failed to query queue");

            let job_unlocked = job
                .as_ref()
                .is_some_and(|(_, locked_by, locked_at)| locked_by.is_none() && locked_at.is_none());
            let queue_unlocked = queue.as_ref().is_some_and(
                |(_, queue_locked_by, queue_locked_at)| {
                    queue_locked_by.is_none() && queue_locked_at.is_none()
                },
            );

            if job_unlocked && queue_unlocked {
                break (job.unwrap(), queue.unwrap());
            }

            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Job and queue should have been unlocked after permanent failure. job={job:?}, queue={queue:?}"
                );
            }

            sleep(Duration::from_millis(50)).await;
        };

        let (_, locked_by, locked_at) = job;
        assert!(
            locked_by.is_none(),
            "Job locked_by should be NULL after permanent failure"
        );
        assert!(
            locked_at.is_none(),
            "Job locked_at should be NULL after permanent failure"
        );

        let (_, queue_locked_by, queue_locked_at) = queue;
        assert!(
            queue_locked_by.is_none(),
            "Queue locked_by should be NULL after permanent failure"
        );
        assert!(
            queue_locked_at.is_none(),
            "Queue locked_at should be NULL after permanent failure"
        );

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}
