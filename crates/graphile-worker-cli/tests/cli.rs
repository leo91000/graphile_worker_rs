use std::fs;
use std::process::{Command, Output};

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

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

#[test]
fn job_key_mode_requires_key() {
    let output = command_output(&["add", "send_email", "--job-key-mode", "replace"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--key"), "stderr was:\n{stderr}");
}

#[test]
fn admin_help_documents_embedded_ui_options() {
    let output = command_output(&["admin", "--help"]);
    let stdout = assert_success(output);

    assert!(stdout.contains("Serve the embedded Leptos admin UI"));
    assert!(stdout.contains("--auth"));
    assert!(stdout.contains("--read-only"));
    assert!(stdout.contains("GRAPHILE_WORKER_ADMIN_PASSWORD"));
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

    let database_name = format!("graphile_worker_cli_test_{}", Uuid::now_v7().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE {database_name}"
    )))
    .execute(&source_pool)
    .await
    .expect("failed to create test database");

    let database_url = database_url_for_test_db(&source_url, &database_name);
    let test_pool = PgPool::connect(&database_url)
        .await
        .expect("failed to connect to test database");

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "migrate",
    ]));
    let migrated = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "migrate",
    ]));
    let migrated: Value = serde_json::from_str(&migrated).expect("migrate output should be JSON");
    assert_eq!(migrated["migrated"], true);

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

    let payload_path = std::env::temp_dir().join(format!("{database_name}_payload.json"));
    fs::write(&payload_path, r#"{"to":"file@example.com"}"#)
        .expect("failed to write CLI payload fixture");
    let payload_path = payload_path
        .to_str()
        .expect("payload fixture path should be UTF-8")
        .to_string();

    let added_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "add",
        "send_email",
        "--payload-file",
        &payload_path,
        "--key",
        "cli-file-key",
    ]));
    let added_job: Value = serde_json::from_str(&added_json).expect("add output should be JSON");
    assert_eq!(added_job["task_identifier"], "send_email");
    assert_eq!(added_job["payload"]["to"], "file@example.com");
    let file_job_id = added_job["id"]
        .as_i64()
        .expect("job id should be an integer");

    let listed = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "list",
    ]));
    let jobs: Vec<Value> = serde_json::from_str(&listed).expect("list output should be JSON");
    assert!(jobs.len() >= 2);
    assert!(jobs.iter().any(|job| job["id"] == file_job_id));
    let queued_job = jobs
        .iter()
        .find(|job| job["queue_name"] == "emails")
        .expect("queued job should be listed");
    assert_eq!(queued_job["task_identifier"], "send_email");
    let job_id = queued_job["id"]
        .as_i64()
        .expect("job id should be an integer");

    let listed_table = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "list",
        "--identifier",
        "send_email",
        "--queue",
        "emails",
    ]));
    assert!(listed_table.contains("id\ttask\tqueue"));
    assert!(listed_table.contains("send_email"));

    let shown = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "show",
        &file_job_id.to_string(),
    ]));
    assert!(shown.contains("payload:"));
    assert!(shown.contains("file@example.com"));

    let stats = assert_success(command_output(&["--database-url", &database_url, "stats"]));
    assert!(stats.contains("total\tready\tscheduled\tlocked\tfailed"));

    let queues = assert_success(command_output(&["--database-url", &database_url, "queues"]));
    assert!(queues.contains("id\tqueue\tjobs"));
    assert!(queues.contains("emails"));

    let workers = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "workers",
    ]));
    assert!(workers.contains("worker_id\tlocked_jobs\tlocked_queues"));

    let invalid_reschedule = command_output(&[
        "--database-url",
        &database_url,
        "reschedule",
        &job_id.to_string(),
    ]);
    assert!(!invalid_reschedule.status.success());
    let stderr = String::from_utf8(invalid_reschedule.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("provide at least one"),
        "stderr was:\n{stderr}"
    );

    for state in ["ready", "scheduled", "locked", "failed"] {
        assert_success(command_output(&[
            "--database-url",
            &database_url,
            "list",
            "--state",
            state,
        ]));
    }

    let invalid_limit = command_output(&["--database-url", &database_url, "list", "--limit=-1"]);
    assert!(!invalid_limit.status.success());
    let stderr = String::from_utf8(invalid_limit.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--limit"), "stderr was:\n{stderr}");

    let invalid_offset = command_output(&["--database-url", &database_url, "list", "--offset=-1"]);
    assert!(!invalid_offset.status.success());
    let stderr = String::from_utf8(invalid_offset.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--offset"), "stderr was:\n{stderr}");

    let missing_show = command_output(&["--database-url", &database_url, "show", "999999999"]);
    assert!(!missing_show.status.success());
    let stderr = String::from_utf8(missing_show.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("failed to get job"),
        "stderr was:\n{stderr}"
    );

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "reschedule",
        &file_job_id.to_string(),
        "--now",
    ]));

    let scheduled_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "add",
        "send_email",
        "--run-at",
        "2999-01-01T00:00:00Z",
    ]));
    let scheduled_job: Value =
        serde_json::from_str(&scheduled_json).expect("scheduled add output should be JSON");
    let scheduled_job_id = scheduled_job["id"]
        .as_i64()
        .expect("scheduled job id should be an integer");
    let scheduled_table = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "list",
        "--state",
        "scheduled",
    ]));
    assert!(scheduled_table.contains("scheduled"));

    let locked_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "add",
        "send_email",
    ]));
    let locked_job: Value =
        serde_json::from_str(&locked_json).expect("locked add output should be JSON");
    let locked_job_id = locked_job["id"]
        .as_i64()
        .expect("locked job id should be an integer");
    sqlx::query(
        "UPDATE graphile_worker._private_jobs SET locked_at = now(), locked_by = $1 WHERE id = $2",
    )
    .bind("cli-worker")
    .bind(locked_job_id)
    .execute(&test_pool)
    .await
    .expect("failed to lock CLI test job");
    let locked_table = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "list",
        "--state",
        "locked",
    ]));
    assert!(locked_table.contains("locked"));

    let workers_with_lock = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "workers",
    ]));
    assert!(workers_with_lock.contains("cli-worker"));

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

    let completed_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "complete",
        &file_job_id.to_string(),
    ]));
    let completed_jobs: Vec<Value> =
        serde_json::from_str(&completed_json).expect("complete output should be JSON");
    assert_eq!(completed_jobs.len(), 1);
    assert_eq!(completed_jobs[0]["id"], file_job_id);

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

    let failed_table = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "list",
        "--state",
        "failed",
    ]));
    assert!(failed_table.contains("failed"));

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "complete",
        &job_id.to_string(),
    ]));
    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "complete",
        &scheduled_job_id.to_string(),
    ]));
    let unlocked = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "force-unlock",
        "cli-worker",
    ]));
    assert!(unlocked.contains("Unlocked 1 worker id"));
    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "complete",
        &locked_job_id.to_string(),
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

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "add",
        "remove_me",
        "--key",
        "cli-remove-key",
    ]));
    let removed = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "remove",
        "cli-remove-key",
    ]));
    assert!(removed.contains("Removed job"));

    assert_success(command_output(&[
        "--database-url",
        &database_url,
        "add",
        "remove_me",
        "--key",
        "cli-remove-json-key",
    ]));
    let removed_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "remove",
        "cli-remove-json-key",
    ]));
    let removed_json: Value =
        serde_json::from_str(&removed_json).expect("remove output should be JSON");
    assert_eq!(removed_json["removed_key"], "cli-remove-json-key");

    let cleanup = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "cleanup",
    ]));
    assert!(cleanup.contains("Cleanup complete"));

    let cleanup_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "cleanup",
        "gc-job-queues",
    ]));
    let cleanup_json: Value =
        serde_json::from_str(&cleanup_json).expect("cleanup output should be JSON");
    assert!(cleanup_json["cleanup_tasks"].is_array());

    let unlocked = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "force-unlock",
        "unknown-worker",
    ]));
    assert!(unlocked.contains("Unlocked 1 worker id"));

    let unlocked_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "force-unlock",
        "unknown-worker-json",
    ]));
    let unlocked_json: Value =
        serde_json::from_str(&unlocked_json).expect("force-unlock output should be JSON");
    assert_eq!(unlocked_json["unlocked_workers"][0], "unknown-worker-json");

    let dry_sweep = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "sweep-stale-workers",
        "--dry-run",
        "--sweep-threshold",
        "1s",
    ]));
    assert!(dry_sweep.contains("No stale workers found"));

    sqlx::query(
        "INSERT INTO graphile_worker._private_workers (id, last_heartbeat_at, metadata) VALUES ($1, now() - interval '1 hour', '{}'::jsonb)",
    )
    .bind("cli-stale-worker")
    .execute(&test_pool)
    .await
    .expect("failed to insert stale worker");

    let non_empty_dry_sweep = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "sweep-stale-workers",
        "--dry-run",
        "--sweep-threshold",
        "1s",
    ]));
    assert!(non_empty_dry_sweep.contains("Would recover 1 worker"));

    let recovered_sweep = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "sweep-stale-workers",
        "--sweep-threshold",
        "1s",
    ]));
    assert!(recovered_sweep.contains("Recovered 0 job"));

    let sweep_json = assert_success(command_output(&[
        "--database-url",
        &database_url,
        "--json",
        "sweep-stale-workers",
        "--dry-run",
    ]));
    let sweep_json: Value = serde_json::from_str(&sweep_json).expect("sweep output should be JSON");
    assert!(sweep_json["worker_ids"].is_array());

    assert_success(command_output_with_database_url_env(
        &database_url,
        &["--json", "stats"],
    ));

    fs::remove_file(&payload_path).expect("failed to remove CLI payload fixture");

    test_pool.close().await;
    source_pool.close().await;
    let source_pool = PgPool::connect(&source_url)
        .await
        .expect("failed to reconnect to source database");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {database_name} WITH (FORCE)"
    )))
    .execute(&source_pool)
    .await
    .expect("failed to drop test database");
}
