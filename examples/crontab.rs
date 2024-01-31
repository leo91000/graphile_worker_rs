use std::str::FromStr;

use graphile_worker::{WorkerContext, WorkerOptions};
use serde::Deserialize;
use sqlx::postgres::PgConnectOptions;
use tracing_subscriber::{
    filter::EnvFilter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

#[derive(Deserialize)]
struct HelloPayload {
    message: String,
}

async fn say_hello(_ctx: WorkerContext, payload: HelloPayload) -> Result<(), ()> {
    println!("Waiting 20 seconds");
    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    println!("Hello {} !", payload.message);
    Ok(())
}

fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    // Log level set to debug except for sqlx set at warn (to not show all sql requests)
    let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
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
        .define_raw_job("say_hello", say_hello)
        .define_raw_job("say_hello_2", say_hello)
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
