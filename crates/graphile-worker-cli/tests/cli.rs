use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sqlx::PgPool;

fn database_url_for_test_db(source_url: &str, name: &str) -> String {
    let mut database_url = source_url.to_string();
    let scheme_end = database_url
        .find("://")
        .expect("DATABASE_URL must have scheme")
        + 3;
    let path_start = database_url[scheme_end..]
        .find('/')
        .map(|offset| scheme_end + offset)
        .expect("DATABASE_URL must include database path");
    let query_start = database_url[path_start..]
        .find('?')
        .map(|offset| path_start + offset)
        .unwrap_or(database_url.len());

    database_url.replace_range(path_start + 1..query_start, name);
    database_url
}

fn command_output(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_graphile-worker"))
        .args(args)
        .output()
        .expect("failed to run graphile-worker")
}

fn command_output_with_database_url_env(database_url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_graphile-worker"))
        .env("DATABASE_URL", database_url)
        .args(args)
        .output()
        .expect("failed to run graphile-worker")
}

fn assert_success(output: Output) -> String {
    if !output.status.success() {
        panic!(
            "command failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

#[tokio::test]
async fn cli_manages_job_lifecycle_with_database_url_flag() {
    let Ok(source_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping CLI database integration test because DATABASE_URL is not set");
        return;
    };
    let source_pool = PgPool::connect(&source_url)
        .await
        .expect("failed to connect to source database");

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let database_name = format!("graphile_worker_cli_test_{}_{}", std::process::id(), suffix);
    sqlx::query(&format!("CREATE DATABASE {database_name}"))
        .execute(&source_pool)
        .await
        .expect("failed to create test database");

    let database_url = database_url_for_test_db(&source_url, &database_name);

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "migrate",
    ]));
    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "add",
        "send_email",
        "--payload",
        r#"{"to":"user@example.com"}"#,
        "--queue",
        "emails",
        "--key",
        "cli-test-key",
    ]));

    let listed = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "list",
    ]));
    let jobs: Vec<Value> = serde_json::from_str(&listed).expect("list output should be JSON");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["task_identifier"], "send_email");
    assert_eq!(jobs[0]["queue_name"], "emails");
    let job_id = jobs[0]["id"].as_i64().expect("job id should be an integer");

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "reschedule",
        &job_id.to_string(),
        "--priority",
        "7",
    ]));

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "fail",
        &job_id.to_string(),
        "--reason",
        "invalid test payload",
    ]));

    let failed = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "list",
        "--state",
        "failed",
    ]));
    let failed_jobs: Vec<Value> =
        serde_json::from_str(&failed).expect("list output should be JSON");
    assert_eq!(failed_jobs.len(), 1);
    assert_eq!(failed_jobs[0]["last_error"], "invalid test payload");
    assert_eq!(failed_jobs[0]["priority"], 7);

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "complete",
        &job_id.to_string(),
    ]));

    let listed_after_complete = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "list",
    ]));
    let jobs_after_complete: Vec<Value> =
        serde_json::from_str(&listed_after_complete).expect("list output should be JSON");
    assert!(jobs_after_complete.is_empty());

    assert_success(command_output_with_database_url_env(
        &database_url,
        &["--json", "stats"],
    ));

    source_pool.close().await;
    let source_pool = PgPool::connect(&source_url)
        .await
        .expect("failed to reconnect to source database");
    sqlx::query(&format!("DROP DATABASE {database_name} WITH (FORCE)"))
        .execute(&source_pool)
        .await
        .expect("failed to drop test database");
}
