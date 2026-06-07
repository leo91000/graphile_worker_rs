use super::*;

#[tokio::test]
async fn worker_options_accepts_job_definitions() {
    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .listen_os_shutdown_signals(false)
            .define_jobs(builder_jobs())
            .init()
            .await
            .expect("Failed to create worker");

        assert!(worker.jobs().contains_key("builder_job"));
        assert!(worker.jobs().contains_key("other_builder_job"));
    })
    .await;
}

#[tokio::test]
async fn worker_options_runs_job_definitions() {
    static DEFINITION_RUN_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Deserialize, Serialize)]
    struct DefinitionRunJob {
        value: u32,
    }

    impl TaskHandler for DefinitionRunJob {
        const IDENTIFIER: &'static str = "definition_run_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            assert_eq!(self.value, 42);
            DEFINITION_RUN_COUNT.increment().await;
        }
    }

    DEFINITION_RUN_COUNT.reset().await;

    with_test_db(|test_db| async move {
        let definition = DefinitionRunJob::definition();
        let _handler = definition.handler();

        let worker = test_db
            .create_worker_options()
            .listen_os_shutdown_signals(false)
            .define_jobs([definition])
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_job(DefinitionRunJob { value: 42 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(DEFINITION_RUN_COUNT.get().await, 1);
    })
    .await;
}

#[test]
fn task_handler_definition_exposes_identifier() {
    assert_eq!(BuilderJob::definition().identifier(), "builder_job");
    assert_eq!(
        JobDefinition::of::<OtherBuilderJob>().identifier(),
        "other_builder_job"
    );
}
