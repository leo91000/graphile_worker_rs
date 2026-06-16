# Job Management

`WorkerUtils` is the operational API for managing Graphile Worker jobs from Rust
code. Use it to schedule work, cancel keyed jobs, complete or fail selected
jobs, reschedule jobs, clean up old metadata, run migrations, and recover locks
left behind by stopped workers.

Most applications get a `WorkerUtils` instance from an initialized worker:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;

let utils = worker.create_utils();
```

Operator tools that do not run a worker can construct utilities directly from a
database handle and schema:

```rust,ignore
use graphile_worker::WorkerUtils;

let utils = WorkerUtils::new(database, "graphile_worker");
utils.migrate().await?;
```

## Scheduling Jobs

Use `add_job` when the task type is known at compile time. The task handler's
`IDENTIFIER` is used as the job identifier, and the payload is serialized to
JSON.

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}

let job = utils
    .add_job(
        SendEmail {
            to: "user@example.com".to_string(),
        },
        JobSpec::default(),
    )
    .await?;
```

Use `add_raw_job` when the identifier is dynamic or when building an operator
tool that schedules jobs without linking the task type.

```rust,ignore
use graphile_worker::JobSpec;
use serde_json::json;

let job = utils
    .add_raw_job(
        "send_email",
        json!({ "to": "user@example.com" }),
        JobSpec::default(),
    )
    .await?;
```

`WorkerUtils` also supports batch insertion:

```rust,ignore
let spec = JobSpec::default();
let jobs = vec![
    (SendEmail { to: "a@example.com".to_string() }, &spec),
    (SendEmail { to: "b@example.com".to_string() }, &spec),
];

let added = utils.add_jobs(&jobs).await?;
```

For mixed identifiers, use `add_raw_jobs` with `RawJobSpec` values:

```rust,ignore
use graphile_worker::{JobSpec, RawJobSpec};
use serde_json::json;

let added = utils
    .add_raw_jobs(&[
        RawJobSpec {
            identifier: "send_email".to_string(),
            payload: json!({ "to": "a@example.com" }),
            spec: JobSpec::default(),
        },
        RawJobSpec {
            identifier: "generate_report".to_string(),
            payload: json!({ "report_id": 42 }),
            spec: JobSpec::default(),
        },
    ])
    .await?;
```

Empty `add_jobs` and `add_raw_jobs` calls return an empty vector without writing
to the database. For large batches, the utility attempts to run `ANALYZE`
on the jobs table after inserting at least 10,000 jobs.

## Job Keys

Set `JobSpec::job_key` to deduplicate work. Adding another job with the same key
updates the existing job instead of creating another row. In the tested behavior,
the later payload replaces the earlier payload and the job revision increments.

```rust,ignore
use graphile_worker::{JobKeyMode, JobSpec};

utils
    .add_job(
        SendEmail {
            to: "user@example.com".to_string(),
        },
        JobSpec {
            job_key: Some("welcome-email:user-123".to_string()),
            job_key_mode: Some(JobKeyMode::Replace),
            ..Default::default()
        },
    )
    .await?;
```

`JobKeyMode::PreserveRunAt` keeps the existing scheduled time when the keyed job
is updated. `JobKeyMode::Replace` replaces it with the new `run_at`.
`JobKeyMode::UnsafeDedupe` is supported for individual `add_job` and
`add_raw_job` calls, but batch insertion rejects it. If a batch contains any
`PreserveRunAt` job key mode, the batch insert applies preserve-run-at behavior
uniformly.

Cancel a keyed job with `remove_job`:

```rust,ignore
utils.remove_job("welcome-email:user-123").await?;
```

## Batch Task Jobs

For handlers that implement `BatchTaskHandler`, `add_batch_job` stores the
payload as a JSON array in a single job row.

```rust,ignore
let job = utils
    .add_batch_job(
        vec![
            SendEmail { to: "a@example.com".to_string() },
            SendEmail { to: "b@example.com".to_string() },
        ],
        JobSpec::default(),
    )
    .await?;
```

The payload list must contain at least one item. If a `before_job_schedule` hook
is installed, it must return a JSON array for batch jobs and must not return an
empty array.

## Completing, Failing, and Rescheduling Jobs

Administrative job actions take job ids and return the jobs that were changed.
Locked jobs are left untouched by `complete_jobs`, `permanently_fail_jobs`, and
`reschedule_jobs`.

```rust,ignore
let completed = utils.complete_jobs(&[101, 102]).await?;

let failed = utils
    .permanently_fail_jobs(&[103], "invalid payload")
    .await?;
```

Permanent failure sets `last_error` to the supplied reason and sets `attempts`
to `max_attempts`.

Use `RescheduleJobOptions` to update only the fields you need:

```rust,ignore
use chrono::Utc;
use graphile_worker::worker_utils::types::RescheduleJobOptions;

let rescheduled = utils
    .reschedule_jobs(
        &[104, 105],
        RescheduleJobOptions {
            run_at: Some(Utc::now() + chrono::Duration::minutes(5)),
            priority: Some(10),
            attempts: Some(0),
            max_attempts: Some(25),
        },
    )
    .await?;
```

If `run_at` is not supplied, the database function schedules matching jobs to
run immediately. `priority`, `attempts`, and `max_attempts` are optional and are
left unchanged when omitted.

## Cleanup

`cleanup` runs one or more maintenance tasks:

```rust,ignore
use graphile_worker::worker_utils::types::CleanupTask;

utils
    .cleanup(&[
        CleanupTask::GcTaskIdentifiers,
        CleanupTask::GcJobQueues,
        CleanupTask::DeletePermanentlyFailedJobs,
    ])
    .await?;
```

The available cleanup tasks are:

- `GcTaskIdentifiers`: removes task identifiers no longer referenced by jobs.
- `GcJobQueues`: removes queue records no longer referenced by jobs.
- `DeletePermanentlyFailedJobs`: deletes unlocked jobs whose attempts have
  reached `max_attempts`.

When utilities come from a worker, `GcTaskIdentifiers` preserves task
identifiers known by that worker and refreshes its task details afterward. This
keeps horizontally scaled workers able to pick up newly scheduled jobs for
registered task handlers after cleanup.

## Worker Recovery Operations

For deployments using worker recovery, `WorkerUtils` can inspect heartbeat
workers and run recovery sweeps manually.

```rust,ignore
use std::time::Duration;

let workers = utils
    .list_active_workers(Duration::from_secs(300))
    .await?;
```

Run a stale-worker sweep to recover jobs locked by workers that stopped
heartbeating, and jobs or queues locked by worker ids that are no longer
registered:

```rust,ignore
use graphile_worker::SweepStaleWorkersOptions;
use std::time::Duration;

let result = utils
    .sweep_stale_workers(SweepStaleWorkersOptions {
        sweep_threshold: Some(Duration::from_secs(300)),
        recovery_delay: Some(Duration::from_secs(30)),
        dry_run: true,
        ..Default::default()
    })
    .await?;
```

Use `dry_run: true` for inspection before making changes. When you need the
sweep to use a specific recovery configuration, call
`sweep_stale_workers_with_config`.

`force_unlock_workers` is a direct unlock tool for known worker ids:

```rust,ignore
utils
    .force_unlock_workers(&["graphile_worker_deadbeef"])
    .await?;
```

It clears locks for jobs and queues held by those workers while leaving other
workers' locks alone.

## Migrations

Operator processes can run schema migrations through utilities:

```rust,ignore
utils.migrate().await?;
```

This is the same migration path exposed by the command-line tool.

## Command-Line Operations

The `graphile-worker` binary wraps the same operational concepts for shell
workflows. It connects with `--database-url` or `DATABASE_URL` and defaults to
the `graphile_worker` schema.

```bash
graphile-worker --database-url postgres://postgres:postgres@localhost/postgres migrate
DATABASE_URL=postgres://postgres:postgres@localhost/postgres graphile-worker add send_email --payload '{"to":"user@example.com"}'
graphile-worker list --state ready
graphile-worker complete 123 124
graphile-worker fail 125 --reason "invalid payload"
graphile-worker reschedule 126 --run-at 2026-01-02T03:04:05Z
graphile-worker remove cli-job-key
graphile-worker cleanup
graphile-worker force-unlock graphile_worker_deadbeef
graphile-worker sweep-stale-workers --sweep-threshold 5m --recovery-delay 30s
graphile-worker sweep-stale-workers --dry-run
```

Use the Rust API from application code and purpose-built operator services. Use
the CLI for ad hoc inspection, maintenance, and recovery tasks.
