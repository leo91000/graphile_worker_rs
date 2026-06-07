use graphile_worker::{IntoTaskHandlerResult, TaskHandler};
use graphile_worker_ctx::WorkerContext;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Deserialize, Serialize)]
pub(crate) struct ExampleTask {
    pub(crate) name: String,
    pub(crate) value: i32,
}

impl TaskHandler for ExampleTask {
    const IDENTIFIER: &'static str = "example_task";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        info!("Processing task: {} with value: {}", self.name, self.value);
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<(), String>(())
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct DatabaseTask {
    pub(crate) query: String,
}

impl TaskHandler for DatabaseTask {
    const IDENTIFIER: &'static str = "database_task";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        info!("Executing database query: {}", self.query);

        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM graphile_worker.jobs")
            .fetch_one(ctx.pg_pool())
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        info!("Current job count in database: {}", row.0);

        Ok::<(), String>(())
    }
}
