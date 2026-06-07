use super::*;

#[tokio::test]
async fn test_before_job_schedule_transform() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();
        let run_plugin = TestHooksPlugin::new();
        let run_counters = run_plugin.counters();

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .add_plugin(run_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let worker_fut = spawn_local(async move {
            worker.run().await.expect("Failed to run worker");
        });

        sleep(Duration::from_millis(100)).await;

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "transform": true
                }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = run_counters.clone();
        wait_for_condition(
            || c.job_complete.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have completed",
        )
        .await;

        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.transformed.load(Ordering::SeqCst), 1);
        assert_eq!(run_counters.job_complete.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
