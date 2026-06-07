#[path = "sendable_worker/http.rs"]
mod http;
#[path = "sendable_worker/tasks.rs"]
mod tasks;

use graphile_worker::Worker;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{error, info};

use http::handle_http_request;
use tasks::{DatabaseTask, ExampleTask};

/// Example demonstrating running both a web server and worker concurrently
/// using tokio::spawn, showcasing that the worker is Send + Sync.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/graphile_worker_test".to_string());

    info!("Creating worker with database URL: {}", database_url);

    let worker = Worker::options()
        .concurrency(4)
        .poll_interval(Duration::from_secs(1))
        .database_url(&database_url)
        .define_job::<ExampleTask>()
        .define_job::<DatabaseTask>()
        .init()
        .await?;

    info!("Worker created successfully");

    let utils = Arc::new(worker.create_utils());
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    info!("HTTP server listening on http://127.0.0.1:3000");

    let worker_handle = tokio::spawn(async move {
        info!("Starting worker...");
        if let Err(e) = worker.run().await {
            error!("Worker error: {:?}", e);
        }
        info!("Worker stopped");
    });

    let utils_clone = utils.clone();
    let server_handle = tokio::spawn(async move {
        info!("Starting HTTP server...");
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from: {}", addr);
                    let utils_for_request = utils_clone.clone();
                    tokio::spawn(async move {
                        handle_http_request(stream, utils_for_request).await;
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    });

    let worker2 = Worker::options()
        .concurrency(2)
        .poll_interval(Duration::from_secs(2))
        .database_url(&database_url)
        .define_job::<ExampleTask>()
        .init()
        .await?;

    let worker2_handle = tokio::spawn(async move {
        info!("Starting secondary worker...");
        if let Err(e) = worker2.run().await {
            error!("Secondary worker error: {:?}", e);
        }
        info!("Secondary worker stopped");
    });

    info!("All services started. The application is now running:");
    info!("- HTTP server on :3000 with job scheduling endpoints:");
    info!("  * GET http://localhost:3000/health - Health check");
    info!("  * POST 'http://localhost:3000/schedule/example?name=test&value=42' - Schedule example task");
    info!("  * POST 'http://localhost:3000/schedule/database?query=SELECT%201' - Schedule database task");
    info!("- Multiple background workers processing jobs");
    info!("- Press Ctrl+C to stop");

    tokio::select! {
        _ = worker_handle => info!("Worker task completed"),
        _ = server_handle => info!("Server task completed"),
        _ = worker2_handle => info!("Secondary worker task completed"),
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }

    info!("Application shutdown complete");
    Ok(())
}
