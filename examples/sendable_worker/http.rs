#[path = "http/query.rs"]
mod query;
#[path = "http/response.rs"]
mod response;
#[path = "http/schedule.rs"]
mod schedule;

use graphile_worker::WorkerUtils;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tracing::info;

use response::write_http_response;
use schedule::{schedule_database_task, schedule_example_task};

pub(crate) async fn handle_http_request(mut stream: TcpStream, utils: Arc<WorkerUtils>) {
    let buf_reader = BufReader::new(&mut stream);
    let mut lines = buf_reader.lines();

    let request_line = match lines.next_line().await {
        Ok(Some(line)) => line,
        _ => {
            let _ = write_http_response(&mut stream, 400, "Bad Request", "Invalid request").await;
            return;
        }
    };

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
            let _ = write_http_response(
                &mut stream,
                200,
                "OK",
                "Worker is running!\n\nAvailable endpoints:\n- GET /health - Health check\n- POST /schedule/example?name=test&value=42 - Schedule example task\n- POST /schedule/database?query=SELECT 1 - Schedule database task",
            )
            .await;
        }
        ("POST", path) if path.starts_with("/schedule/example") => {
            schedule_example_task(&mut stream, &utils, path).await;
        }
        ("POST", path) if path.starts_with("/schedule/database") => {
            schedule_database_task(&mut stream, &utils, path).await;
        }
        _ => {
            let _ = write_http_response(&mut stream, 404, "Not Found", "Endpoint not found").await;
        }
    }
}
