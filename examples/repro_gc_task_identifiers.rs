use std::env;

use chrono::{DateTime, Utc};
use graphile_worker::{
    worker_utils::CleanupTask, IntoTaskHandlerResult, JobSpecBuilder, WorkerContext,
    WorkerContextExt, WorkerOptions,
};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow};
use tracing_subscriber::{
    filter::EnvFilter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

const SCHEMA: &str = "example_repro_gc_task_identifiers";

fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter_layer = EnvFilter::try_new("info,sqlx=warn").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

#[derive(Deserialize, Serialize, Clone)]
struct CleanupJob;

impl TaskHandler for CleanupJob {
    const IDENTIFIER: &'static str = "cleanup";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        ctx.cleanup(&[CleanupTask::GcTaskIdentifiers])
            .await
            .map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    }
}

#[derive(Deserialize, Serialize, Clone)]
struct EventHandlerJob;

impl TaskHandler for EventHandlerJob {
    const IDENTIFIER: &'static str = "event_handler";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("event_handler ran");
        Ok::<(), String>(())
    }
}

#[derive(FromRow, Debug)]
struct TaskRow {
    id: i32,
    identifier: String,
}

#[derive(FromRow, Debug)]
struct JobRow {
    id: i64,
    task_identifier: String,
    task_id: i32,
    run_at: DateTime<Utc>,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    is_available: bool,
}

async fn print_tasks(pg_pool: &sqlx::PgPool, label: &str) -> Result<(), sqlx::Error> {
    let sql = format!(
        "select id, identifier from {schema}._private_tasks order by id",
        schema = SCHEMA
    );
    let tasks: Vec<TaskRow> = sqlx::query_as(&sql).fetch_all(pg_pool).await?;
    println!("\n{label} tasks:");
    for task in tasks {
        println!("  - id={} identifier={}", task.id, task.identifier);
    }
    Ok(())
}

async fn print_jobs(pg_pool: &sqlx::PgPool, label: &str) -> Result<(), sqlx::Error> {
    let sql = format!(
        "\
        select jobs.id,
               tasks.identifier as task_identifier,
               jobs.task_id,
               jobs.run_at,
               jobs.locked_at,
               jobs.locked_by,
               jobs.is_available
          from {schema}._private_jobs as jobs
          join {schema}._private_tasks as tasks on tasks.id = jobs.task_id
         order by jobs.id\
        ",
        schema = SCHEMA
    );
    let jobs: Vec<JobRow> = sqlx::query_as(&sql).fetch_all(pg_pool).await?;
    println!("\n{label} jobs:");
    for job in jobs {
        println!(
            "  - id={} task_identifier={} task_id={} run_at={} locked_at={:?} locked_by={:?} is_available={}",
            job.id,
            job.task_identifier,
            job.task_id,
            job.run_at,
            job.locked_at,
            job.locked_by,
            job.is_available
        );
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_logs();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let drop_schema_sql = format!("drop schema if exists {schema} cascade", schema = SCHEMA);
    sqlx::query(&drop_schema_sql).execute(&pg_pool).await?;

    let worker = WorkerOptions::default()
        .schema(SCHEMA)
        .concurrency(1)
        .pg_pool(pg_pool.clone())
        .define_job::<CleanupJob>()
        .define_job::<EventHandlerJob>()
        .init()
        .await?;

    let utils = worker.create_utils();

    utils
        .add_job(CleanupJob, JobSpecBuilder::new().build())
        .await?;

    println!("Running cleanup job...");
    worker.run_once().await?;
    print_tasks(&pg_pool, "After cleanup").await?;

    utils
        .add_job(EventHandlerJob, JobSpecBuilder::new().build())
        .await?;

    print_jobs(&pg_pool, "After enqueue event_handler").await?;

    println!("Attempting to process event_handler with stale task IDs...");
    worker.run_once().await?;
    print_jobs(&pg_pool, "After stale worker run_once").await?;

    println!("Starting a fresh worker to show it processes the job...");
    let fresh_worker = WorkerOptions::default()
        .schema(SCHEMA)
        .concurrency(1)
        .pg_pool(pg_pool.clone())
        .define_job::<CleanupJob>()
        .define_job::<EventHandlerJob>()
        .init()
        .await?;

    fresh_worker.run_once().await?;
    print_jobs(&pg_pool, "After fresh worker run_once").await?;

    Ok(())
}
