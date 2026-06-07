use super::*;

#[tokio::test]
async fn register_identifiers() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let database = test_db.database.clone();
        let worker_handle = spawn_local(async move {
            Worker::options()
                .database(database)
                .concurrency(3)
                .define_job::<Job3>()
                .with_cron(
                    r#"
                        0 */4 * * * do_it ?fill=1d
                    "#,
                )
                .expect("Invalid crontab")
                .init()
                .await
                .expect("Failed to create worker")
                .run()
                .await
                .expect("Failed to run worker");
        });

        let mut known_crontabs = test_db.get_known_crontabs().await;
        let start = Instant::now();
        // Wait for known crontabs to be registered
        while known_crontabs.is_empty() {
            if start.elapsed().as_secs() > 5 {
                panic!("Crontab should have been registered by now");
            }
            known_crontabs = test_db.get_known_crontabs().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let known_crontab = &known_crontabs[0];
        assert_eq!(known_crontab.identifier(), "do_it");
        assert!(known_crontab.known_since() < &chrono::Utc::now());
        assert!(known_crontab.last_execution().is_none());

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);

        worker_handle.abort();
    })
    .await;
}
