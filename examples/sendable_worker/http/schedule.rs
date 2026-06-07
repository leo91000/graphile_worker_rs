use graphile_worker::{JobSpecBuilder, WorkerUtils};
use tokio::net::TcpStream;
use tracing::error;

use super::query::parse_query_params;
use super::response::write_http_response;
use crate::tasks::{DatabaseTask, ExampleTask};

pub(super) async fn schedule_example_task(stream: &mut TcpStream, utils: &WorkerUtils, path: &str) {
    let Some(query) = path.split('?').nth(1) else {
        let _ = write_http_response(
            stream,
            400,
            "Bad Request",
            "Missing query parameters. Use: ?name=test&value=42",
        )
        .await;
        return;
    };

    let params = parse_query_params(query);
    let name = params
        .get("name")
        .map_or("default".to_string(), Clone::clone);
    let value = params
        .get("value")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    match utils
        .add_job(ExampleTask { name, value }, Default::default())
        .await
    {
        Ok(_) => {
            let _ = write_http_response(stream, 200, "OK", "Example task scheduled successfully!")
                .await;
        }
        Err(e) => {
            error!("Failed to schedule example task: {:?}", e);
            let _ = write_http_response(
                stream,
                500,
                "Internal Server Error",
                "Failed to schedule task",
            )
            .await;
        }
    }
}

pub(super) async fn schedule_database_task(
    stream: &mut TcpStream,
    utils: &WorkerUtils,
    path: &str,
) {
    let Some(query) = path.split('?').nth(1) else {
        let _ = write_http_response(
            stream,
            400,
            "Bad Request",
            "Missing query parameters. Use: ?query=SELECT COUNT(*) FROM users",
        )
        .await;
        return;
    };

    let params = parse_query_params(query);
    let Some(sql_query) = params.get("query") else {
        let _ = write_http_response(
            stream,
            400,
            "Bad Request",
            "Missing query parameter. Use: ?query=SELECT 1",
        )
        .await;
        return;
    };

    let task = DatabaseTask {
        query: sql_query.clone(),
    };
    let job_spec = JobSpecBuilder::new()
        .priority(10)
        .run_at(chrono::Utc::now() + chrono::Duration::seconds(10))
        .job_key(format!("db_task_{}", chrono::Utc::now().timestamp()))
        .build();

    match utils.add_job(task, job_spec).await {
        Ok(_) => {
            let _ = write_http_response(
                stream,
                200,
                "OK",
                "Database task scheduled to run in 10 seconds with high priority!",
            )
            .await;
        }
        Err(e) => {
            error!("Failed to schedule database task: {:?}", e);
            let _ = write_http_response(
                stream,
                500,
                "Internal Server Error",
                "Failed to schedule task",
            )
            .await;
        }
    }
}
