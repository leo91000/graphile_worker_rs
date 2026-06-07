use super::*;

#[tokio::test]
async fn local_queue_with_forbidden_flags_uses_direct_fetch() {
    with_test_db(|test_db| async move {
        FLAGGED_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(FlaggedJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        for i in 4..=6 {
            utils
                .add_job(
                    FlaggedJob { id: i },
                    JobSpec {
                        flags: Some(vec!["special".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job with flag");
        }

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(3)
                    .local_queue(LocalQueueConfig::builder().size(10).build())
                    .add_forbidden_flag("special")
                    .define_job::<FlaggedJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        wait_for_counter(
            &FLAGGED_CALL_COUNT,
            3,
            Duration::from_secs(5),
            Duration::from_millis(100),
            "Jobs without flag should have been executed by now",
        )
        .await;

        sleep(Duration::from_millis(500)).await;

        assert_eq!(
            FLAGGED_CALL_COUNT.get().await,
            3,
            "Only 3 jobs without the special flag should have been executed"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert_eq!(
            remaining_jobs.len(),
            3,
            "3 jobs with the special flag should remain in the queue"
        );

        worker_fut.abort();
    })
    .await;
}
