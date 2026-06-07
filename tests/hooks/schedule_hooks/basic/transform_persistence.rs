use super::*;

#[tokio::test]
async fn test_before_job_schedule_transform_payload_stored_in_db() {
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

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 42,
                    "transform": true
                }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        assert_eq!(schedule_counters.transformed.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);

        let job_payload = &jobs[0].payload;
        assert_eq!(job_payload.get("value").unwrap().as_u64().unwrap(), 42);
        assert!(job_payload.get("transform").unwrap().as_bool().unwrap());
        assert!(
            job_payload.get("transformed").unwrap().as_bool().unwrap(),
            "Transformed field should be added by hook"
        );
    })
    .await;
}
