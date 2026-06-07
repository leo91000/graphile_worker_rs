use std::str::FromStr;

use graphile_worker::WorkerOptions;
use sqlx::postgres::PgConnectOptions;

#[path = "hooks/logging.rs"]
mod logging;
#[path = "hooks/metrics.rs"]
mod metrics;
#[path = "hooks/process_data.rs"]
mod process_data;
#[path = "hooks/validation.rs"]
mod validation;

use logging::enable_logs;
use metrics::MetricsPlugin;
use process_data::ProcessData;
use validation::ValidationPlugin;

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
