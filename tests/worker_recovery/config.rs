use super::*;

#[test]
fn recovery_config_builders_enable_recovery_and_store_values() {
    let config = WorkerRecoveryConfig::default()
        .heartbeat_interval(Duration::from_millis(10))
        .sweep_interval(Duration::from_millis(20))
        .sweep_threshold(Duration::from_millis(30))
        .recovery_delay(Duration::from_millis(40))
        .shutdown_grace_period(Duration::from_millis(50))
        .shutdown_recovery_delay(Duration::from_millis(60))
        .resilient_sweep_threshold_multiplier(7)
        .resilient_job_flags(vec!["custom_resilient".to_string()]);

    assert!(config.enabled);
    assert_eq!(config.heartbeat_interval, Duration::from_millis(10));
    assert_eq!(config.sweep_interval, Duration::from_millis(20));
    assert_eq!(config.sweep_threshold, Duration::from_millis(30));
    assert_eq!(config.recovery_delay, Duration::from_millis(40));
    assert_eq!(config.shutdown_grace_period, Duration::from_millis(50));
    assert_eq!(config.shutdown_recovery_delay, Duration::from_millis(60));
    assert_eq!(config.resilient_sweep_threshold_multiplier, 7);
    assert_eq!(config.resilient_job_flags, vec!["custom_resilient"]);

    let disabled = config.enabled(false);
    assert!(!disabled.enabled);
}

#[test]
fn job_resilient_flag_detection_requires_truthy_configured_flag() {
    let config = WorkerRecoveryConfig::default();

    let no_flags = Job::builder().build();
    assert!(!job_has_resilient_flag(&no_flags, &config));

    let false_flag = Job::builder()
        .flags(serde_json::json!({ INFRASTRUCTURE_RESILIENT_FLAG: false }))
        .build();
    assert!(!job_has_resilient_flag(&false_flag, &config));

    let truthy_flag = Job::builder()
        .flags(serde_json::json!({ INFRASTRUCTURE_RESILIENT_FLAG: true }))
        .build();
    assert!(job_has_resilient_flag(&truthy_flag, &config));

    let custom_config =
        WorkerRecoveryConfig::default().resilient_job_flags(vec!["custom_resilient".to_string()]);
    let custom_flag = Job::builder()
        .flags(serde_json::json!({ "custom_resilient": true }))
        .build();
    assert!(job_has_resilient_flag(&custom_flag, &custom_config));
}

#[tokio::test]
async fn default_recovery_config_does_not_register_worker() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = tokio::task::spawn(async move { worker_for_run.run().await });

        sleep(Duration::from_millis(250)).await;

        let registered_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM graphile_worker._private_workers")
                .fetch_one(&test_db.test_pool)
                .await
                .expect("failed to count registered workers");

        worker.request_shutdown();
        worker_fut
            .await
            .expect("worker task should not panic")
            .expect("worker should shut down cleanly");

        assert_eq!(registered_count, 0);
    })
    .await;
}
