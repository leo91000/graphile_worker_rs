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
