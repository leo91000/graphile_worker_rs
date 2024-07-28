use std::str::FromStr;

use graphile_worker::{IntoTaskHandlerResult, TaskHandler};
use graphile_worker::{JobSpec, WorkerContext, WorkerOptions};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use tracing_subscriber::{filter::EnvFilter, prelude::*, util::SubscriberInitExt};

fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    // Log level set to debug except for sqlx set at warn (to not show all sql requests)
    let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}

impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Waiting 20 seconds");
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        println!("Hello {} !", self.message);
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
        .concurrency(2)
        .schema("example_simple_worker")
        .define_job::<SayHello>()
        .pg_pool(pg_pool)
        .init()
        .await
        .unwrap();

    let helpers = worker.create_utils();

    // Schedule 10 jobs
    for i in 0..10 {
        helpers
            .add_job(
                SayHello {
                    message: format!("world {}", i),
                },
                JobSpec::default(),
            )
            .await
            .unwrap();
    }

    worker.run_once().await.unwrap();
}
