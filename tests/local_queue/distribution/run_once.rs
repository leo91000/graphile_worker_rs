use super::*;

#[tokio::test]
async fn local_queue_works_with_run_once() {
    with_test_db(|test_db| async move {
        RUN_ONCE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(RunOnceJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(3)
            .define_job::<RunOnceJob>()
            .init()
            .await
            .expect("Failed to create worker");

        worker.run_once().await.expect("Failed to run_once");

        assert_eq!(
            RUN_ONCE_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed with run_once"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert_eq!(
            remaining_jobs.len(),
            0,
            "No jobs should remain after run_once"
        );
    })
    .await;
}
