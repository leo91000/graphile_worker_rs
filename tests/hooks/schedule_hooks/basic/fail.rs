use super::*;

#[tokio::test]
async fn test_before_job_schedule_fail() {
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

        let worker_fut = spawn_local(async move {
            worker.run().await.expect("Failed to run worker");
        });

        sleep(Duration::from_millis(100)).await;

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "fail_schedule": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.failed.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
