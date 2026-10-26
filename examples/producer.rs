use graphile_worker::{JobSpec, WorkerUtils};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&database_url).await?;
    let utils = WorkerUtils::new(pool, "graphile_worker");

    // Run migrations here for this standalone example. In a deployment, the
    // schema can instead be migrated before starting the producer hosts.
    utils.migrate().await?;

    utils
        .add_raw_job(
            "send_email",
            json!({ "to": "user@example.com", "subject": "Welcome" }),
            JobSpec::default(),
        )
        .await?;

    println!("Queued send_email; a separate worker will process it.");
    Ok(())
}
