use super::*;

#[tokio::test]
async fn worker_options_accepts_explicit_recovery_config() {
    with_test_db(|test_db| async move {
        let recovery_config = WorkerRecoveryConfig::default()
            .enabled(true)
            .recovery_delay(Duration::from_millis(123));

        let worker = test_db
            .create_worker_options()
            .listen_os_shutdown_signals(false)
            .worker_recovery(recovery_config.clone())
            .define_job::<BuilderJob>()
            .init()
            .await
            .expect("Failed to create worker");

        assert!(worker.recovery_config().enabled);
        assert_eq!(
            worker.recovery_config().recovery_delay,
            recovery_config.recovery_delay
        );
    })
    .await;
}

#[tokio::test]
async fn worker_options_accepts_shutdown_config_without_enabling_recovery() {
    with_test_db(|test_db| async move {
        let shutdown_config = WorkerShutdownConfig::default()
            .grace_period(Duration::from_millis(50))
            .interrupted_job_retry_delay(Duration::from_millis(60));

        let worker = test_db
            .create_worker_options()
            .worker_shutdown(shutdown_config.clone())
            .listen_os_shutdown_signals(false)
            .define_job::<BuilderJob>()
            .init()
            .await
            .expect("Failed to create worker");

        assert!(!worker.recovery_config().enabled);
        assert!(!worker.shutdown_config().listen_os_shutdown_signals);
        assert_eq!(
            worker.shutdown_config().grace_period,
            shutdown_config.grace_period
        );
        assert_eq!(
            worker.shutdown_config().interrupted_job_retry_delay,
            shutdown_config.interrupted_job_retry_delay
        );
    })
    .await;
}
