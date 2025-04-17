use std::str::FromStr;

use chrono::{offset::Utc, Duration};
use graphile_worker::{IntoTaskHandlerResult, JobSpecBuilder, WorkerContext, WorkerOptions};
use graphile_worker_task_handler::TaskHandler;
use rand::Rng;
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

#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}

impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Hello {} !", self.message);
        // 30% chance to succeed to test retry
        let mut rng = rand::rng();
        if rng.random_range(0..100) < 70 {
            return Err("Failed".to_string());
        }
        Ok(())
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

    helpers
        .add_job(
            SayHello {
                message: "world".to_string(),
            },
            JobSpecBuilder::new()
                .run_at(Utc::now() + Duration::seconds(10))
                .build(),
        )
        .await
        .unwrap();
    worker.run().await.unwrap();
}
