use super::*;

#[tokio::test]
async fn test_before_job_schedule_receives_correct_context() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = IdentifierCapturingPlugin::new();
        let counters = plugin.counters();

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({ "value": 1 }),
                JobSpec::builder()
                    .queue_name("test_queue")
                    .priority(10)
                    .build(),
            )
            .await
            .expect("Failed to add job");

        let captured_id = counters.captured_identifier.lock().unwrap().clone();
        assert_eq!(captured_id, Some("test_hooks_job".to_string()));

        let captured_queue = counters.captured_spec_queue.lock().unwrap().clone();
        assert_eq!(captured_queue, Some("test_queue".to_string()));

        let captured_priority = *counters.captured_spec_priority.lock().unwrap();
        assert_eq!(captured_priority, Some(10));
    })
    .await;
}
#[tokio::test]
async fn test_before_job_schedule_with_typed_add_job() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .define_job::<TypedScheduleJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_job(
                TypedScheduleJob {
                    message: "hello".to_string(),
                    transform: true,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.transformed.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task_identifier, "typed_schedule_job");

        let payload = &jobs[0].payload;
        assert_eq!(payload.get("message").unwrap().as_str().unwrap(), "hello");
        assert!(
            payload.get("transformed").unwrap().as_bool().unwrap(),
            "Hook should have added transformed field"
        );
    })
    .await;
}
