use super::*;

#[tokio::test]
async fn worker_options_initializes_from_database_url() {
    with_test_db(|test_db| async move {
        let database_url = database_url_for_test_db(&test_db.name);

        let worker = WorkerOptions::default()
            .database_url(&database_url)
            .max_pg_conn(1)
            .listen_os_shutdown_signals(false)
            .with_cron(Cron::every_minute::<BuilderJob>())
            .with_cron("* * * * * builder_job")
            .expect("Failed to add second crontab")
            .use_local_time(true)
            .on(JobStart, |_ctx| async {})
            .define_job::<BuilderJob>()
            .init()
            .await
            .expect("Failed to create worker");

        assert!(*worker.use_local_time());
        assert_eq!(worker.crontabs().len(), 2);
        assert_eq!(worker.concurrency(), &num_cpus::get());
    })
    .await;
}

#[tokio::test]
async fn worker_options_requires_database_url_or_pool() {
    let result = WorkerOptions::default()
        .listen_os_shutdown_signals(false)
        .init()
        .await;

    match result {
        Err(graphile_worker::WorkerBuildError::MissingDatabaseUrl) => {}
        Err(error) => panic!("Expected missing database URL error, got {error:?}"),
        Ok(_) => panic!("Expected worker initialization to fail"),
    }
}
