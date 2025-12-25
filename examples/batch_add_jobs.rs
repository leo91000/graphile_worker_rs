use std::str::FromStr;

use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, JobSpecBuilder, RawJobSpec, WorkerContext, WorkerOptions,
};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};
use serde_json::json;
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

#[derive(Clone, Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!(
            "Sending email to {} with subject: {}",
            self.to, self.subject
        );
        Ok::<(), String>(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct ProcessPayment {
    user_id: u32,
    amount: u32,
}

impl TaskHandler for ProcessPayment {
    const IDENTIFIER: &'static str = "process_payment";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!(
            "Processing payment of ${} for user {}",
            self.amount, self.user_id
        );
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
        .schema("example_batch_add_jobs")
        .define_job::<SendEmail>()
        .define_job::<ProcessPayment>()
        .pg_pool(pg_pool)
        .init()
        .await
        .unwrap();

    let utils = worker.create_utils();

    println!("\n=== Batch Add Jobs Example ===\n");

    println!("1. Adding multiple emails with add_jobs (typed, same job type)...");
    let spec = JobSpec::default();
    let emails = vec![
        (
            SendEmail {
                to: "alice@example.com".into(),
                subject: "Welcome!".into(),
            },
            &spec,
        ),
        (
            SendEmail {
                to: "bob@example.com".into(),
                subject: "Welcome!".into(),
            },
            &spec,
        ),
        (
            SendEmail {
                to: "carol@example.com".into(),
                subject: "Welcome!".into(),
            },
            &spec,
        ),
    ];

    let jobs = utils.add_jobs::<SendEmail>(&emails).await.unwrap();
    println!("   Added {} email jobs in a single batch\n", jobs.len());

    println!("2. Adding mixed job types with add_raw_jobs (heterogeneous)...");
    let mixed_jobs = vec![
        RawJobSpec {
            identifier: "send_email".into(),
            payload: json!({ "to": "dave@example.com", "subject": "Notification" }),
            spec: JobSpec::default(),
        },
        RawJobSpec {
            identifier: "process_payment".into(),
            payload: json!({ "user_id": 123, "amount": 50 }),
            spec: JobSpecBuilder::new().priority(10).build(),
        },
        RawJobSpec {
            identifier: "process_payment".into(),
            payload: json!({ "user_id": 456, "amount": 100 }),
            spec: JobSpec::default(),
        },
    ];

    let jobs = utils.add_raw_jobs(&mixed_jobs).await.unwrap();
    println!("   Added {} mixed jobs in a single batch\n", jobs.len());

    println!("Processing jobs...\n");
    worker.run().await.unwrap();
}
