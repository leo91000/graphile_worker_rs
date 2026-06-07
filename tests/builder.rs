use graphile_worker::{
    Cron, IntoTaskHandlerResult, JobDefinition, JobSpec, JobStart, TaskHandler, WorkerContext,
    WorkerOptions, WorkerRecoveryConfig, WorkerShutdownConfig,
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

#[path = "builder/cron.rs"]
mod cron;
#[path = "builder/database_url.rs"]
mod database_url;
#[path = "builder/job_definitions.rs"]
mod job_definitions;
#[path = "builder/recovery.rs"]
mod recovery;
#[path = "builder/schema.rs"]
mod schema;
