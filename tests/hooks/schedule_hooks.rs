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

#[tokio::test]
async fn test_before_job_schedule_skip() {
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
                    "skip_schedule": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.skipped.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

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

#[tokio::test]
async fn test_before_job_schedule_multiple_plugins_chain_transforms() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counters = Arc::new(ChainedTransformCounters::default());
        let plugin1 = ChainedTransformPlugin1 {
            counters: counters.clone(),
        };
        let plugin2 = ChainedTransformPlugin2 {
            counters: counters.clone(),
        };

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(plugin1)
            .add_plugin(plugin2)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({ "value": 1 }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        assert_eq!(counters.plugin1_calls.load(Ordering::SeqCst), 1);
        assert_eq!(counters.plugin2_calls.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);

        let payload = &jobs[0].payload;
        assert!(
            payload.get("plugin1_processed").unwrap().as_bool().unwrap(),
            "Plugin 1 should have processed"
        );
        assert!(
            payload.get("plugin2_processed").unwrap().as_bool().unwrap(),
            "Plugin 2 should have processed"
        );
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_skip_stops_chain() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let second_plugin_calls = Arc::new(AtomicU32::new(0));
        let second_plugin = SecondPluginCounter {
            calls: second_plugin_calls.clone(),
        };

        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(SkippingFirstPlugin)
            .add_plugin(second_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "skip_in_first": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(
            second_plugin_calls.load(Ordering::SeqCst),
            0,
            "Second plugin should not be called when first plugin skips"
        );

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_and_before_job_run_both_called() {
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
                serde_json::json!({ "value": 1 }),
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

        assert_eq!(
            schedule_counters.before_schedule.load(Ordering::SeqCst),
            1,
            "Schedule hook should be called"
        );
        assert_eq!(
            run_counters.before_job_run.load(Ordering::SeqCst),
            1,
            "Run hook should be called"
        );
        assert_eq!(
            run_counters.job_start.load(Ordering::SeqCst),
            1,
            "Job should have started"
        );
        assert_eq!(
            run_counters.job_complete.load(Ordering::SeqCst),
            1,
            "Job should have completed"
        );

        worker_fut.abort();
    })
    .await;
}
