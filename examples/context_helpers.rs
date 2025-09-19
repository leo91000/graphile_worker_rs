use std::str::FromStr;

use chrono::{offset::Utc, Duration};
use graphile_worker::{
    IntoTaskHandlerResult, JobSpecBuilder, WorkerContext, WorkerContextExt, WorkerOptions,
};
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

#[derive(Deserialize, Serialize, Clone)]
struct SendWs {
    request_id: String,
}

impl TaskHandler for SendWs {
    const IDENTIFIER: &'static str = "send_ws";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        // 1) pretend to send a websocket message here
        println!("[send_ws] sent request {}", self.request_id);

        // 2) schedule a follow-up check in 10 seconds using the new helpers
        ctx.add_job(
            CheckWs {
                request_id: self.request_id.clone(),
            },
            JobSpecBuilder::new()
                .run_at(Utc::now() + Duration::seconds(10))
                .build(),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }
}

#[derive(Deserialize, Serialize, Clone)]
struct CheckWs {
    request_id: String,
}

impl TaskHandler for CheckWs {
    const IDENTIFIER: &'static str = "check_ws";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        // 3) check for a response/result here
        println!("[check_ws] checking response for {}", self.request_id);
        Ok::<(), String>(())
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
        .schema("example_context_helpers")
        .define_job::<SendWs>()
        .define_job::<CheckWs>()
        .pg_pool(pg_pool)
        .init()
        .await
        .unwrap();

    // Kick off a first SendWs job. The follow-up CheckWs is scheduled by the handler via ctx.add_job.
    let utils = worker.create_utils();
    utils
        .add_job(
            SendWs {
                request_id: "abc-123".to_string(),
            },
            JobSpecBuilder::new().build(),
        )
        .await
        .unwrap();

    worker.run().await.unwrap();
}
