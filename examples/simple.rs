use std::str::FromStr;

use archimedes::{WorkerContext, WorkerOptions};
use serde::Deserialize;
use sqlx::postgres::PgConnectOptions;

#[derive(Deserialize)]
struct HelloPayload {
    message: String,
}

async fn say_hello(_ctx: WorkerContext, payload: HelloPayload) -> anyhow::Result<()> {
    println!("Hello {} !", payload.message);
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "TRACE");
    tracing_subscriber::fmt::init();

    let pg_options = PgConnectOptions::from_str("postgres://postgres:root@localhost:5432")?;

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect_with(pg_options)
        .await?;

    WorkerOptions::default()
        .concurrency(2)
        .schema("example_simple_worker")
        .define_job("say_hello", say_hello)
        .pg_pool(pg_pool)
        .init()
        .await?
        .run()
        .await?;

    Ok(())
}
