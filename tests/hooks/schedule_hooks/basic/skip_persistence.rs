use super::*;

#[tokio::test]
async fn test_before_job_schedule_skip_no_job_in_db() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "skip_schedule": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(schedule_counters.skipped.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0, "Skipped job should not be in database");
    })
    .await;
}
