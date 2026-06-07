use super::*;

#[tokio::test]
async fn local_queue_processes_jobs_with_refetch_delay() {
    with_test_db(|test_db| async move {
        REFETCH_WITH_JOBS_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(RefetchDelayWithJobsJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .poll_interval(Duration::from_millis(200))
                    .local_queue(
                        LocalQueueConfig::default()
                            .with_size(10)
                            .with_refetch_delay(
                                RefetchDelayConfig::default()
                                    .with_duration(Duration::from_millis(100))
                                    .with_threshold(3)
                                    .with_max_abort_threshold(10),
                            ),
                    )
                    .define_job::<RefetchDelayWithJobsJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        wait_for_counter(
            &REFETCH_WITH_JOBS_CALL_COUNT,
            5,
            Duration::from_secs(10),
            Duration::from_millis(100),
            "Jobs should have been executed",
        )
        .await;

        assert_eq!(
            REFETCH_WITH_JOBS_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed with refetch delay configured"
        );

        worker_fut.abort();
    })
    .await;
}
