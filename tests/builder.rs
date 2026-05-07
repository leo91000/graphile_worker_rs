use graphile_worker::{IntoTaskHandlerResult, JobStart, TaskHandler, WorkerContext, WorkerOptions};
use serde::{Deserialize, Serialize};

use crate::helpers::with_test_db;

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

#[tokio::test]
async fn worker_options_initializes_from_database_url() {
    with_test_db(|test_db| async move {
        let database_url = database_url_for_test_db(&test_db.name);

        let worker = WorkerOptions::default()
            .database_url(&database_url)
            .max_pg_conn(1)
            .listen_os_shutdown_signals(false)
            .with_crontab("* * * * * builder_job")
            .expect("Failed to add first crontab")
            .with_crontab("* * * * * builder_job")
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
