use super::*;

#[tokio::test]
async fn worker_options_initializes_schema_that_requires_quoting() {
    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .schema("Case-Schema")
            .listen_os_shutdown_signals(false)
            .define_job::<BuilderJob>()
            .init()
            .await
            .expect("Failed to create worker");

        assert_eq!(worker.schema().escaped(), "\"Case-Schema\"");
    })
    .await;
}
