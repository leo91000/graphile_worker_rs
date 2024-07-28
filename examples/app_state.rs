use graphile_worker::IntoTaskHandlerResult;
use graphile_worker::WorkerContext;
use graphile_worker::WorkerOptions;
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use tracing_subscriber::{
    filter::EnvFilter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    // Log level set to debug except for sqlx set at warn (to not show all sql requests)
    let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

#[derive(Clone, Debug)]
struct AppState {
    run_count: Arc<AtomicUsize>,
}

impl AppState {
    fn new() -> Self {
        Self {
            run_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn increment_run_count(&self) -> usize {
        self.run_count.fetch_add(1, SeqCst)
    }
}

#[derive(Deserialize, Serialize)]
struct ShowRunCount;

impl TaskHandler for ShowRunCount {
    const IDENTIFIER: &'static str = "show_run_count";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let app_state = ctx.get_ext::<AppState>().unwrap();
        let run_count = app_state.increment_run_count();
        println!("Run count: {run_count}");
    }
}

#[tokio::main]
async fn main() {
    enable_logs();

    let pg_options = PgConnectOptions::from_str("postgres://postgres:root@localhost:5432").unwrap();

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect_with(pg_options)
        .await
        .unwrap();

    let worker = WorkerOptions::default()
        .concurrency(10)
        .schema("example_simple_worker")
        .define_job::<ShowRunCount>()
        .pg_pool(pg_pool)
        .add_extension(AppState::new())
        .with_crontab(
            r#"
                * * * * * show_run_count ?fill=1h
            "#,
        )
        .expect("Failed to parse crontab")
        .init()
        .await
        .unwrap();

    let utils = worker.create_utils();

    for _ in 0..10 {
        utils
            .add_job(ShowRunCount, Default::default())
            .await
            .unwrap();
    }

    worker.run().await.unwrap();
}
