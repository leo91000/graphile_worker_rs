use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker::{
    HookRegistry, IntoTaskHandlerResult, LocalQueueConfig, LocalQueueGetJobsComplete,
    LocalQueueInit, LocalQueueRefetchDelayAbort, LocalQueueRefetchDelayExpired,
    LocalQueueRefetchDelayStart, LocalQueueReturnJobs, LocalQueueSetMode, Plugin,
    RefetchDelayConfig, WorkerContext, WorkerOptions,
};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use tracing_subscriber::{
    filter::EnvFilter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter_layer = EnvFilter::try_new("info,graphile_worker=debug").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

#[derive(Debug, Default)]
struct LocalQueueMonitorPlugin {
    jobs_fetched: AtomicUsize,
    jobs_returned: AtomicUsize,
    refetch_delays_started: AtomicUsize,
    refetch_delays_aborted: AtomicUsize,
}

impl Plugin for LocalQueueMonitorPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let plugin = Arc::new(self);

        hooks.on(LocalQueueInit, async |ctx| {
            println!(
                "[LocalQueueMonitor] LocalQueue initialized for worker {}",
                ctx.worker_id
            );
        });

        hooks.on(LocalQueueSetMode, async |ctx| {
            println!(
                "[LocalQueueMonitor] Mode changed: {:?} -> {:?}",
                ctx.old_mode, ctx.new_mode
            );
        });

        {
            let plugin = plugin.clone();
            hooks.on(LocalQueueGetJobsComplete, move |ctx| {
                let plugin = plugin.clone();
                async move {
                    plugin
                        .jobs_fetched
                        .fetch_add(ctx.jobs_count, Ordering::Relaxed);
                    println!(
                        "[LocalQueueMonitor] Batch fetched {} jobs (total: {})",
                        ctx.jobs_count,
                        plugin.jobs_fetched.load(Ordering::Relaxed)
                    );
                }
            });
        }

        {
            let plugin = plugin.clone();
            hooks.on(LocalQueueReturnJobs, move |ctx| {
                let plugin = plugin.clone();
                async move {
                    plugin.jobs_returned.fetch_add(ctx.jobs_count, Ordering::Relaxed);
                    println!(
                        "[LocalQueueMonitor] Returned {} jobs to database (TTL expired or shutdown)",
                        ctx.jobs_count
                    );
                }
            });
        }

        {
            let plugin = plugin.clone();
            hooks.on(LocalQueueRefetchDelayStart, move |ctx| {
                let plugin = plugin.clone();
                async move {
                    plugin.refetch_delays_started.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[LocalQueueMonitor] Refetch delay started: {:?} (threshold: {}, abort_threshold: {})",
                        ctx.duration, ctx.threshold, ctx.abort_threshold
                    );
                }
            });
        }

        {
            let plugin = plugin.clone();
            hooks.on(LocalQueueRefetchDelayAbort, move |ctx| {
                let plugin = plugin.clone();
                async move {
                    plugin
                        .refetch_delays_aborted
                        .fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[LocalQueueMonitor] Refetch delay aborted! count={} >= abort_threshold={}",
                        ctx.count, ctx.abort_threshold
                    );
                }
            });
        }

        hooks.on(LocalQueueRefetchDelayExpired, async |_ctx| {
            println!("[LocalQueueMonitor] Refetch delay expired naturally");
        });
    }
}

#[derive(Deserialize, Serialize)]
struct ProcessItem {
    item_id: u32,
}

impl TaskHandler for ProcessItem {
    const IDENTIFIER: &'static str = "process_item";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("Processed item {}", self.item_id);
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
        .concurrency(4)
        .schema("example_local_queue")
        .define_job::<ProcessItem>()
        .pg_pool(pg_pool)
        .add_plugin(LocalQueueMonitorPlugin::default())
        .local_queue(
            LocalQueueConfig::default()
                .with_size(50)
                .with_ttl(Duration::from_secs(60))
                .with_refetch_delay(
                    RefetchDelayConfig::default()
                        .with_duration(Duration::from_millis(100))
                        .with_threshold(5)
                        .with_max_abort_threshold(20),
                ),
        )
        .init()
        .await
        .unwrap();

    let utils = worker.create_utils();

    println!("\n=== LocalQueue Example ===");
    println!("LocalQueue batch-fetches jobs to reduce database load.");
    println!("Configuration:");
    println!("  - Size: 50 (max jobs to fetch per batch)");
    println!("  - TTL: 60s (return unfetched jobs after this time)");
    println!("  - Refetch delay: 100ms (pause before next fetch when queue is low)");
    println!("  - Refetch threshold: 5 (trigger delay when fewer jobs fetched)");
    println!("  - Abort threshold: 20 (cancel delay after this many pulses)\n");

    println!("Adding 20 jobs...\n");

    for i in 1..=20 {
        utils
            .add_job(ProcessItem { item_id: i }, Default::default())
            .await
            .unwrap();
    }

    println!("Jobs added. Watch the LocalQueue behavior:\n");
    println!("1. Jobs are batch-fetched from the database");
    println!("2. When batch size < threshold, refetch delay activates");
    println!("3. New job notifications can abort the delay early\n");

    worker.run().await.unwrap();
}
