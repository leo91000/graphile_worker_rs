use graphile_worker::{
    Cron, IntoTaskHandlerResult, JobDefinition, JobSpec, JobStart, TaskHandler, WorkerContext,
    WorkerOptions, WorkerRecoveryConfig,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

fn database_url_for_test_db(name: &str) -> String {
    let mut database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let scheme_end = database_url
        .find("://")
        .expect("DATABASE_URL must have scheme")
        + 3;
    let path_start = database_url[scheme_end..]
        .find('/')
        .map(|offset| scheme_end + offset)
        .expect("DATABASE_URL must include database path");
    let query_start = database_url[path_start..]
        .find('?')
        .map(|offset| path_start + offset)
        .unwrap_or(database_url.len());

    database_url.replace_range(path_start + 1..query_start, name);
    database_url
}

#[derive(Deserialize, Serialize)]
struct BuilderJob;

impl TaskHandler for BuilderJob {
    const IDENTIFIER: &'static str = "builder_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
}

#[derive(Deserialize, Serialize)]
struct OtherBuilderJob;

impl TaskHandler for OtherBuilderJob {
    const IDENTIFIER: &'static str = "other_builder_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
}

fn builder_jobs() -> [JobDefinition; 2] {
    [BuilderJob::definition(), OtherBuilderJob::definition()]
}

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

        assert_eq!(worker.escaped_schema(), "\"Case-Schema\"");
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

#[test]
fn task_handler_definition_exposes_identifier() {
    assert_eq!(BuilderJob::definition().identifier(), "builder_job");
    assert_eq!(
        JobDefinition::of::<OtherBuilderJob>().identifier(),
        "other_builder_job"
    );
}

#[test]
fn worker_options_with_cron_accepts_crontab_text() {
    let _ = WorkerOptions::default()
        .with_cron("* * * * * builder_job")
        .expect("Failed to parse crontab string literal");

    let _ = WorkerOptions::default()
        .with_cron(String::from("* * * * * builder_job"))
        .expect("Failed to parse owned crontab string");
}

#[test]
#[allow(deprecated)]
fn worker_options_deprecated_with_crontab_still_parses_text() {
    let _ = WorkerOptions::default()
        .with_crontab("* * * * * builder_job")
        .expect("Failed to parse deprecated crontab string");
}
