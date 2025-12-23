use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use graphile_worker::{
    BeforeJobRun, HookRegistry, HookResult, IntoTaskHandlerResult, JobComplete, JobFail, JobStart,
    Plugin, WorkerContext, WorkerOptions, WorkerShutdown, WorkerStart,
};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use tracing_subscriber::{
    filter::EnvFilter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

#[derive(Debug)]
struct MetricsPlugin {
    jobs_started: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
}

impl MetricsPlugin {
    fn new() -> Self {
        Self {
            jobs_started: AtomicU64::new(0),
            jobs_completed: AtomicU64::new(0),
            jobs_failed: AtomicU64::new(0),
        }
    }
}

impl Plugin for MetricsPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            println!("[MetricsPlugin] Worker {} started", ctx.worker_id);
        });

        let jobs_started = Arc::new(self.jobs_started);
        let jobs_completed = Arc::new(self.jobs_completed);
        let jobs_failed = Arc::new(self.jobs_failed);

        {
            let jobs_started = jobs_started.clone();
            let jobs_completed = jobs_completed.clone();
            let jobs_failed = jobs_failed.clone();
            hooks.on(WorkerShutdown, move |ctx| {
                let jobs_started = jobs_started.clone();
                let jobs_completed = jobs_completed.clone();
                let jobs_failed = jobs_failed.clone();
                async move {
                    println!(
                        "[MetricsPlugin] Worker {} shutting down (reason: {:?})",
                        ctx.worker_id, ctx.reason
                    );
                    println!(
                        "Stats: started={}, completed={}, failed={}",
                        jobs_started.load(Ordering::Relaxed),
                        jobs_completed.load(Ordering::Relaxed),
                        jobs_failed.load(Ordering::Relaxed)
                    );
                }
            });
        }

        {
            let jobs_started = jobs_started.clone();
            hooks.on(JobStart, move |ctx| {
                let jobs_started = jobs_started.clone();
                async move {
                    jobs_started.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} started (task: {})",
                        ctx.job.id(),
                        ctx.job.task_identifier()
                    );
                }
            });
        }

        {
            let jobs_completed = jobs_completed.clone();
            hooks.on(JobComplete, move |ctx| {
                let jobs_completed = jobs_completed.clone();
                async move {
                    jobs_completed.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} completed in {:?}",
                        ctx.job.id(),
                        ctx.duration
                    );
                }
            });
        }

        {
            let jobs_failed = jobs_failed.clone();
            hooks.on(JobFail, move |ctx| {
                let jobs_failed = jobs_failed.clone();
                async move {
                    jobs_failed.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} failed: {} (will_retry: {})",
                        ctx.job.id(),
                        ctx.error,
                        ctx.will_retry
                    );
                }
            });
        }
    }
}

struct ValidationPlugin;

impl Plugin for ValidationPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobRun, async |ctx| {
            let should_skip = ctx
                .payload
                .get("skip")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let should_fail = ctx
                .payload
                .get("force_fail")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if should_skip {
                println!(
                    "[ValidationPlugin] Skipping job {} due to skip flag",
                    ctx.job.id()
                );
                return HookResult::Skip;
            }

            if should_fail {
                println!(
                    "[ValidationPlugin] Failing job {} due to force_fail flag",
                    ctx.job.id()
                );
                return HookResult::Fail("Forced failure by validation plugin".into());
            }

            HookResult::Continue
        });
    }
}

#[derive(Deserialize, Serialize)]
struct ProcessData {
    value: i32,
    #[serde(default)]
    skip: bool,
    #[serde(default)]
    force_fail: bool,
}

impl TaskHandler for ProcessData {
    const IDENTIFIER: &'static str = "process_data";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Processing data with value: {}", self.value);
        Ok::<(), String>(())
    }
}

fn get_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:root@localhost:5432".to_string())
}

#[tokio::main]
async fn main() {
    enable_logs();

    let pg_options = PgConnectOptions::from_str(&get_database_url()).unwrap();

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect_with(pg_options)
        .await
        .unwrap();

    let worker = WorkerOptions::default()
        .concurrency(2)
        .schema("example_hooks")
        .define_job::<ProcessData>()
        .pg_pool(pg_pool)
        .add_plugin(MetricsPlugin::new())
        .add_plugin(ValidationPlugin)
        .init()
        .await
        .unwrap();

    let utils = worker.create_utils();

    utils
        .add_job(
            ProcessData {
                value: 1,
                skip: false,
                force_fail: false,
            },
            Default::default(),
        )
        .await
        .unwrap();

    utils
        .add_job(
            ProcessData {
                value: 2,
                skip: true,
                force_fail: false,
            },
            Default::default(),
        )
        .await
        .unwrap();

    utils
        .add_job(
            ProcessData {
                value: 3,
                skip: false,
                force_fail: true,
            },
            Default::default(),
        )
        .await
        .unwrap();

    utils
        .add_job(
            ProcessData {
                value: 4,
                skip: false,
                force_fail: false,
            },
            Default::default(),
        )
        .await
        .unwrap();

    println!("\nAdded 4 jobs:");
    println!("  - Job 1: Normal processing");
    println!("  - Job 2: Will be skipped by ValidationPlugin");
    println!("  - Job 3: Will be failed by ValidationPlugin");
    println!("  - Job 4: Normal processing\n");

    worker.run().await.unwrap();
}
