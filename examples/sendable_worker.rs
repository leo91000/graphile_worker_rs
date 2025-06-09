use graphile_worker::{IntoTaskHandlerResult, JobSpecBuilder, TaskHandler, Worker, WorkerUtils};
use graphile_worker_ctx::WorkerContext;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

/// Example task that can be processed by the worker
#[derive(Deserialize, Serialize)]
struct ExampleTask {
    name: String,
    value: i32,
}

impl TaskHandler for ExampleTask {
    const IDENTIFIER: &'static str = "example_task";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        info!("Processing task: {} with value: {}", self.name, self.value);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok::<(), String>(())
    }
}

/// Another example task
#[derive(Deserialize, Serialize)]
struct DatabaseTask {
    query: String,
}

impl TaskHandler for DatabaseTask {
    const IDENTIFIER: &'static str = "database_task";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        info!("Executing database query: {}", self.query);

        // Example of using the database connection from context
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM graphile_worker.jobs")
            .fetch_one(ctx.pg_pool())
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        info!("Current job count in database: {}", row.0);

        Ok::<(), String>(())
    }
}

/// HTTP request handler that processes simple GET requests
async fn handle_http_request(mut stream: TcpStream, utils: Arc<WorkerUtils>) {
    let buf_reader = BufReader::new(&mut stream);
    let mut lines = buf_reader.lines();

    // Read the first line (request line)
    let request_line = match lines.next_line().await {
        Ok(Some(line)) => line,
        _ => {
            let _ = write_http_response(&mut stream, 400, "Bad Request", "Invalid request").await;
            return;
        }
    };

    // Parse the request
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 3 {
        let _ = write_http_response(&mut stream, 400, "Bad Request", "Invalid request line").await;
        return;
    }

    let method = parts[0];
    let path = parts[1];

    info!("Received {} request for {}", method, path);

    match (method, path) {
        ("GET", "/") | ("GET", "/health") => {
            let _ = write_http_response(&mut stream, 200, "OK", "Worker is running!\n\nAvailable endpoints:\n- GET /health - Health check\n- POST /schedule/example?name=test&value=42 - Schedule example task\n- POST /schedule/database?query=SELECT 1 - Schedule database task").await;
        }
        ("POST", path) if path.starts_with("/schedule/example") => {
            if let Some(query) = path.split('?').nth(1) {
                let params = parse_query_params(query);
                let name = params
                    .get("name")
                    .map_or("default".to_string(), |v| v.clone());
                let value = params
                    .get("value")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);

                let task = ExampleTask { name, value };

                match utils.add_job(task, Default::default()).await {
                    Ok(_) => {
                        let _ = write_http_response(
                            &mut stream,
                            200,
                            "OK",
                            "Example task scheduled successfully!",
                        )
                        .await;
                    }
                    Err(e) => {
                        error!("Failed to schedule example task: {:?}", e);
                        let _ = write_http_response(
                            &mut stream,
                            500,
                            "Internal Server Error",
                            "Failed to schedule task",
                        )
                        .await;
                    }
                }
            } else {
                let _ = write_http_response(
                    &mut stream,
                    400,
                    "Bad Request",
                    "Missing query parameters. Use: ?name=test&value=42",
                )
                .await;
            }
        }
        ("POST", path) if path.starts_with("/schedule/database") => {
            if let Some(query) = path.split('?').nth(1) {
                let params = parse_query_params(query);
                if let Some(sql_query) = params.get("query") {
                    let task = DatabaseTask {
                        query: sql_query.clone(),
                    };

                    // Schedule with higher priority and run in 10 seconds
                    let job_spec = JobSpecBuilder::new()
                        .priority(10)
                        .run_at(chrono::Utc::now() + chrono::Duration::seconds(10))
                        .job_key(format!("db_task_{}", chrono::Utc::now().timestamp()))
                        .build();

                    match utils.add_job(task, job_spec).await {
                        Ok(_) => {
                            let _ = write_http_response(
                                &mut stream,
                                200,
                                "OK",
                                "Database task scheduled to run in 10 seconds with high priority!",
                            )
                            .await;
                        }
                        Err(e) => {
                            error!("Failed to schedule database task: {:?}", e);
                            let _ = write_http_response(
                                &mut stream,
                                500,
                                "Internal Server Error",
                                "Failed to schedule task",
                            )
                            .await;
                        }
                    }
                } else {
                    let _ = write_http_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        "Missing query parameter. Use: ?query=SELECT 1",
                    )
                    .await;
                }
            } else {
                let _ = write_http_response(
                    &mut stream,
                    400,
                    "Bad Request",
                    "Missing query parameters. Use: ?query=SELECT COUNT(*) FROM users",
                )
                .await;
            }
        }
        _ => {
            let _ = write_http_response(&mut stream, 404, "Not Found", "Endpoint not found").await;
        }
    }
}

/// Write an HTTP response
async fn write_http_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
        status_code,
        status_text,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

/// Parse query parameters from a query string
fn parse_query_params(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter_map(|param| {
            let mut parts = param.split('=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => {
                    Some((simple_url_decode(key), simple_url_decode(value)))
                }
                _ => None,
            }
        })
        .collect()
}

/// Simple URL decoder for basic use cases
fn simple_url_decode(s: &str) -> String {
    s.replace("%20", " ")
        .replace("%21", "!")
        .replace("%22", "\"")
        .replace("%23", "#")
        .replace("%24", "$")
        .replace("%25", "%")
        .replace("%26", "&")
        .replace("%27", "'")
        .replace("%28", "(")
        .replace("%29", ")")
        .replace("%2A", "*")
        .replace("%2B", "+")
        .replace("%2C", ",")
        .replace("%2F", "/")
        .replace("%3A", ":")
        .replace("%3B", ";")
        .replace("%3D", "=")
        .replace("%3F", "?")
        .replace("%40", "@")
        .replace("%5B", "[")
        .replace("%5D", "]")
}

/// Example demonstrating running both a web server and worker concurrently
/// using tokio::spawn, showcasing that the worker is now Send + Sync
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/graphile_worker_test".to_string());

    info!("Creating worker with database URL: {}", database_url);

    // Create the worker
    let worker = Worker::options()
        .concurrency(4)
        .poll_interval(Duration::from_secs(1))
        .database_url(&database_url)
        .define_job::<ExampleTask>()
        .define_job::<DatabaseTask>()
        .init()
        .await?;

    info!("Worker created successfully");

    // Create WorkerUtils for job scheduling
    let utils = Arc::new(worker.create_utils());

    // Create a simple HTTP server for scheduling jobs
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    info!("HTTP server listening on http://127.0.0.1:3000");

    // Spawn the worker on a separate task
    // This demonstrates that the worker is now Send + Sync
    let worker_handle = tokio::spawn(async move {
        info!("Starting worker...");
        if let Err(e) = worker.run().await {
            error!("Worker error: {:?}", e);
        }
        info!("Worker stopped");
    });

    // Spawn the HTTP server on another task
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

    // Example of creating additional workers that can be spawned
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

    // Wait for all tasks to complete (or be interrupted)
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
