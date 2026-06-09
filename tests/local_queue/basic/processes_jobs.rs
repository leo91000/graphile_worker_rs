use super::*;

#[tokio::test]
async fn local_queue_processes_jobs_correctly() {
    with_test_db(|test_db| async move {
        LOCAL_QUEUE_JOB_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(3)
                    .local_queue(LocalQueueConfig::builder().size(10).build())
                    .define_job::<LocalQueueJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        for i in 1..=5 {
            utils
                .add_job(LocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");

            wait_for_counter(
                &LOCAL_QUEUE_JOB_CALL_COUNT,
                i,
                Duration::from_secs(5),
                Duration::from_millis(100),
                "Job should have been executed by now",
            )
            .await;

            assert_eq!(
                LOCAL_QUEUE_JOB_CALL_COUNT.get().await,
                i,
                "Job should have been executed {} times",
                i
            );
        }

        wait_for_jobs(
            &test_db,
            Duration::from_secs(5),
            Duration::from_millis(50),
            "All jobs should be removed after completion",
            |jobs| jobs.is_empty(),
        )
        .await;

        assert_eq!(
            LOCAL_QUEUE_JOB_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}
