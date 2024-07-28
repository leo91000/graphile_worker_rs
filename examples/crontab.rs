use graphile_worker::IntoTaskHandlerResult;
use std::str::FromStr;

use graphile_worker::{TaskHandler, WorkerContext, WorkerOptions};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use tracing_subscriber::{prelude::*, EnvFilter};

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

#[derive(Deserialize, Serialize)]
struct SayHello2 {
    message: String,
}

impl TaskHandler for SayHello2 {
    const IDENTIFIER: &'static str = "say_hello_2";

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

    WorkerOptions::default()
        .concurrency(10)
        .schema("example_simple_worker")
        .define_job::<SayHello>()
        .define_job::<SayHello2>()
        .pg_pool(pg_pool)
        // Run say_hello every two minutes with a backfill of 10 minutes
        .with_crontab(
            r#"
                */2 * * * * say_hello ?fill=10m&job_key=say_hello_dedupe&job_key_mode=preserve_run_at {message:"Crontab"}
            "#,
        )
        .unwrap()
        .init()
        .await
        .expect("Failed to init worker")
        .run()
        .await
        .expect("Unexpected worker error");
}
