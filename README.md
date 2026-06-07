# Graphile Worker RS

[![Codecov](https://codecov.io/github/leo91000/graphile_worker_rs/coverage.svg?branch=main)](https://codecov.io/gh/leo91000/graphile_worker_rs)
[![Crates.io](https://img.shields.io/crates/v/graphile-worker.svg)](https://crates.io/crates/graphile-worker)
[![Documentation](https://img.shields.io/docsrs/graphile_worker)](https://docs.rs/graphile_worker/)
[![MIT License](https://img.shields.io/crates/l/graphile-worker.svg)](LICENSE.md)

A powerful PostgreSQL-backed job queue for Rust applications, based on [Graphile Worker](https://github.com/graphile/worker). 
This is a complete Rust rewrite that offers excellent performance, reliability, and a convenient API.

## Overview

Graphile Worker RS allows you to run jobs (such as sending emails, performing calculations, generating PDFs) in the background, 
so your HTTP responses and application code remain fast and responsive. It's ideal for any PostgreSQL-backed Rust application.

Key highlights:
- **High performance**: Uses PostgreSQL's `SKIP LOCKED` for efficient job fetching
- **Low latency**: Typically under 3ms from task schedule to execution using `LISTEN`/`NOTIFY`
- **Reliable**: Automatically retries failed jobs with exponential backoff
- **Flexible**: Supports scheduled jobs, task queues, cron-like recurring tasks, and more
- **Type-safe**: Uses Rust's type system to ensure job payloads match their handlers

## Differences from Node.js version

This port is mostly compatible with the original Graphile Worker, meaning you can run it side by side with the Node.js version.
The key differences are:

- In the Node.js version, each process has its own worker_id. In the Rust version, there is only one worker_id, and jobs are processed in your async runtime thread

## Installation

Add the library to your project:

```bash
cargo add graphile_worker
```

Tokio is the default runtime:

```toml
graphile_worker = { version = "0.11.4", features = ["tls-rustls"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
futures = "0.3"
```

To use async-std instead, disable default features and enable `runtime-async-std`.
Applications using `#[async_std::main]` also need async-std's `attributes`
feature:

```toml
graphile_worker = { version = "0.11.4", default-features = false, features = ["runtime-async-std", "tls-rustls"] }
async-std = { version = "1", features = ["attributes"] }
futures = "0.3"
```

Runtime-specific code is intentionally kept behind adapter crates and
`graphile_worker_runtime`. Core worker modules use that runtime facade instead
of calling Tokio or async-std directly, so the same worker logic can run on the
enabled runtime feature.

## Getting Started

### 1. Define a Task

A task consists of a struct that implements the `TaskHandler` trait. Each task has:
- A struct with `Serialize`/`Deserialize` for the payload
- A unique identifier string
- An async `run` method that contains the task's logic

```rust,ignore
use serde::{Deserialize, Serialize};
use graphile_worker::{WorkerContext, TaskHandler, IntoTaskHandlerResult};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
    body: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending email to {} with subject '{}'", self.to, self.subject);
        // Email sending logic would go here
        Ok::<(), String>(())
    }
}
```

### 2. Configure and Run the Worker

Set up the worker with your configuration options and run it. Use the entrypoint
for the runtime you enabled:

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_worker().await
}
```

```rust,ignore
#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_worker().await
}
```

The worker setup itself is runtime-neutral:

```rust,ignore
async fn run_worker() -> Result<(), Box<dyn std::error::Error>> {
    // Create a PostgreSQL connection pool
    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost/mydb")
        .await?;

    // Initialize and run the worker
    graphile_worker::WorkerOptions::default()
        .concurrency(5)                 // Process up to 5 jobs concurrently
        .schema("graphile_worker")      // Use this PostgreSQL schema
        .define_job::<SendEmail>()      // Register the task handler
        .pg_pool(pg_pool)               // Provide the database connection
        .init()                         // Initialize the worker
        .await?
        .run()                          // Start processing jobs
        .await?;

    Ok(())
}
```

For larger applications, modules can expose their jobs as reusable definitions
and register them together:

```rust,ignore
use graphile_worker::{JobDefinition, TaskHandler};

pub fn jobs() -> [JobDefinition; 2] {
    [
        SendEmail::definition(),
        SendDailyReport::definition(),
    ]
}

graphile_worker::WorkerOptions::default()
    .define_jobs(jobs())
    // ... other configuration
    .init()
    .await?;
```

#### Custom shutdown handling

Graphile Worker installs OS-level signal handlers (like `SIGINT`/`SIGTERM`) so
it can shut down gracefully when you press Ctrl+C. If your application already
owns the shutdown lifecycle, disable the built-in listeners and call
`Worker::request_shutdown()` when your orchestrator asks the worker to stop:

```rust,ignore
let shutdown = graphile_worker::WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(false);

let worker = graphile_worker::WorkerOptions::default()
    .worker_shutdown(shutdown) // prevent installing Ctrl+C handlers
    // ... other configuration
    .init()
    .await?;

let run_loop = worker.run();
tokio::pin!(run_loop);

tokio::select! {
    // Main worker loop
    result = &mut run_loop => result?,
    // Notify the worker when the host framework wants to stop
    () = on_shutdown() => {
        worker.request_shutdown();
        run_loop.await?; // drain gracefully before returning
    }
}
```

### Worker Recovery

Worker recovery is an opt-in feature for deployments where a job may remain
locked after a process crash, network partition, forced abort, or orchestrator
shutdown. When enabled, each worker records heartbeats in PostgreSQL and a
background sweeper recovers jobs locked by workers that have stopped
heartbeating.

```rust,ignore
use graphile_worker::{WorkerOptions, WorkerRecoveryConfig, WorkerShutdownConfig};
use std::time::Duration;

let recovery = WorkerRecoveryConfig::default()
    .enabled(true)
    .heartbeat_interval(Duration::from_secs(30))
    .sweep_interval(Duration::from_secs(60))
    .sweep_threshold(Duration::from_secs(300))
    .recovery_delay(Duration::from_secs(30));

let shutdown = WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(true)
    .grace_period(Duration::from_secs(5))
    .interrupted_job_retry_delay(Duration::from_secs(30));

let worker = WorkerOptions::default()
    .worker_recovery(recovery)
    .worker_shutdown(shutdown)
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

The convenience setters on `WorkerOptions` also enable recovery:

```rust,ignore
let worker = WorkerOptions::default()
    .heartbeat_interval(Duration::from_secs(30))
    .sweep_interval(Duration::from_secs(60))
    .sweep_threshold(Duration::from_secs(300))
    .recovery_delay(Duration::from_secs(30))
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Recovery behavior:

- Recovery is disabled by default to avoid changing existing deployments.
- Enabled workers register in `_private_workers` and refresh
  `last_heartbeat_at` at `heartbeat_interval`.
- The sweeper runs at `sweep_interval`, detects workers older than
  `sweep_threshold`, and also detects orphan locks whose worker id is no longer
  registered.
- Sweeps use a transaction-scoped PostgreSQL advisory lock so only one sweeper
  performs recovery at a time.
- Recovered jobs are unlocked, their attempt count is decremented back, their
  queue lock is released, and their `run_at` is delayed by `recovery_delay`.
- Jobs aborted during graceful shutdown use `WorkerShutdownConfig` instead of
  normal failure backoff, so shutdown does not consume a retry attempt. This
  shutdown behavior is available even when automatic worker recovery is disabled.
- Jobs with the `infrastructure_resilient` flag use the resilient threshold
  multiplier before being considered stale, which is useful for long-running
  infrastructure jobs.

Manual recovery is available through `WorkerUtils`, even when automatic
recovery is disabled:

```rust,ignore
use graphile_worker::SweepStaleWorkersOptions;
use std::time::Duration;

let result = worker.create_utils()
    .sweep_stale_workers(SweepStaleWorkersOptions {
        sweep_threshold: Some(Duration::from_secs(300)),
        recovery_delay: Some(Duration::from_secs(30)),
        dry_run: false,
        ..Default::default()
    })
    .await?;

println!(
    "Recovered {} job(s) from {:?}",
    result.recovered_count,
    result.worker_ids
);
```

`sweep_stale_workers` uses the default recovery configuration for omitted
thresholds. Use `sweep_stale_workers_with_config` when a manual sweep should
reuse a custom `WorkerRecoveryConfig`, for example resilient job flags or a
custom resilient threshold multiplier.

Recovery can be customized with the `JobRecovery` hook:

```rust,ignore
use graphile_worker::{
    FailureReason, HookRegistry, JobRecovery, JobRecoveryResult, Plugin,
};

struct RecoveryPolicy;

impl Plugin for RecoveryPolicy {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(JobRecovery, |ctx| async move {
            match ctx.reason {
                FailureReason::WorkerCrashed => JobRecoveryResult::Default,
                FailureReason::ShutdownAborted => JobRecoveryResult::Default,
                _ => JobRecoveryResult::FailWithBackoff,
            }
        });
    }
}
```

Hook actions can keep the default recovery behavior, reschedule to an explicit
time and attempt count, apply normal failure backoff, or skip recovery for that
job.

### 3. Schedule Jobs

#### Option A: Schedule a job via SQL

Connect to your database and run the following SQL:

```sql
SELECT graphile_worker.add_job(
    'send_email',
    json_build_object(
        'to', 'user@example.com',
        'subject', 'Welcome to our app!',
        'body', 'Thanks for signing up.'
    )
);
```

#### Option B: Schedule a job from Rust

```rust,ignore
// Get a WorkerUtils instance to manage jobs
let utils = worker.create_utils();

// Type-safe method (recommended):
utils.add_job(
    SendEmail {
        to: "user@example.com".to_string(),
        subject: "Welcome to our app!".to_string(),
        body: "Thanks for signing up.".to_string(),
    },
    Default::default(), // Use default job options
).await?;

// Or use the raw method when type isn't available:
utils.add_raw_job(
    "send_email",
    serde_json::json!({
        "to": "user@example.com",
        "subject": "Welcome to our app!",
        "body": "Thanks for signing up."
    }),
    Default::default(),
).await?;
```

#### Option C: Batch job scheduling

For efficiency when adding many jobs at once, use batch methods:

```rust,ignore
// Batch add jobs of the same type (type-safe)
let spec = JobSpec::default();
utils.add_jobs::<SendEmail>(&[
    (SendEmail { to: "user1@example.com".into(), subject: "Hello".into(), body: "...".into() }, &spec),
    (SendEmail { to: "user2@example.com".into(), subject: "Hello".into(), body: "...".into() }, &spec),
    (SendEmail { to: "user3@example.com".into(), subject: "Hello".into(), body: "...".into() }, &spec),
]).await?;

// Batch add jobs of different types (dynamic)
utils.add_raw_jobs(&[
    RawJobSpec {
        identifier: "send_email".into(),
        payload: serde_json::json!({ "to": "user@example.com", "subject": "Hi" }),
        spec: JobSpec::default(),
    },
    RawJobSpec {
        identifier: "process_payment".into(),
        payload: serde_json::json!({ "user_id": 123, "amount": 50 }),
        spec: JobSpec::default(),
    },
]).await?;
```

#### Option D: Batch job processing

Batch jobs store a JSON array in a single job. Batch handlers can return
per-item results, allowing the worker to remove successful items and retry only
the failed items.

```rust,ignore
use graphile_worker::{
    BatchTaskHandler, IntoBatchTaskHandlerResult, JobKeyMode, JobSpecBuilder, WorkerContext,
    WorkerOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct PendingNotification {
    user_id: String,
    message_id: String,
}

impl BatchTaskHandler for PendingNotification {
    const IDENTIFIER: &'static str = "send_notifications";

    async fn run_batch(
        items: Vec<Self>,
        _ctx: WorkerContext,
    ) -> impl IntoBatchTaskHandlerResult {
        futures::future::join_all(items.into_iter().map(|item| async move {
            send_notification(item).await.map_err(|error| error.to_string())
        }))
        .await
    }
}

let worker = WorkerOptions::default()
    .define_batch_job::<PendingNotification>()
    // ... other configuration
    .init()
    .await?;

worker.create_utils()
    .add_batch_job(
        vec![
            PendingNotification { user_id: "1".into(), message_id: "a".into() },
            PendingNotification { user_id: "1".into(), message_id: "b".into() },
        ],
        JobSpecBuilder::new()
            .job_key("notifications:1")
            .job_key_mode(JobKeyMode::PreserveRunAt)
            .build(),
    )
    .await?;
```

## Advanced Features

### Shared Application State

You can provide shared state to all your tasks using extensions:

```rust,ignore
use serde::{Deserialize, Serialize};
use graphile_worker::{WorkerContext, TaskHandler, IntoTaskHandlerResult};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering::SeqCst}};

// Define your shared state
#[derive(Clone, Debug)]
struct AppState {
    db_client: Arc<DatabaseClient>,
    api_key: String,
    counter: Arc<AtomicUsize>,
}

// Example database client (just for demonstration)
struct DatabaseClient;
impl DatabaseClient {
    fn new() -> Self { Self }
    async fn find_user(&self, _user_id: &str) -> Result<(), String> { Ok(()) }
}

#[derive(Deserialize, Serialize)]
struct ProcessUserTask {
    user_id: String,
}

impl TaskHandler for ProcessUserTask {
    const IDENTIFIER: &'static str = "process_user";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        // Access the shared state in your task
        let app_state = ctx.get_ext::<AppState>().unwrap();
        let count = app_state.counter.fetch_add(1, SeqCst);
        
        // Use shared resources
        app_state.db_client.find_user(&self.user_id).await?;
        
        println!("Processed user {}, task count: {}", self.user_id, count);
        Ok::<(), String>(())
    }
}

// Add the extension when configuring the worker
let app_state = AppState {
    db_client: Arc::new(DatabaseClient::new()),
    api_key: "secret_key".to_string(),
    counter: Arc::new(AtomicUsize::new(0)),
};

graphile_worker::WorkerOptions::default()
    .add_extension(app_state)
    .define_job::<ProcessUserTask>()
    // ... other configuration
    .init()
    .await?;
```

### Scheduling Options

You can customize how and when jobs run with the `JobSpec` builder:

```rust,ignore
use graphile_worker::{JobSpecBuilder, JobKeyMode};
use chrono::Utc;

// Schedule a job to run after 5 minutes with high priority
let job_spec = JobSpecBuilder::new()
    .run_at(Utc::now() + chrono::Duration::minutes(5))
    .priority(10)
    .job_key("welcome_email_user_123")  // Unique identifier for deduplication
    .job_key_mode(JobKeyMode::Replace)  // Replace existing jobs with this key
    .max_attempts(5)                    // Max retry attempts (default is 25)
    .build();

utils.add_job(SendEmail { /* ... */ }, job_spec).await?;
```

### Job Queues

Jobs with the same queue name run in series (one after another) rather than in parallel:

```rust,ignore
// These jobs will run one after another, not concurrently
let spec1 = JobSpecBuilder::new()
    .queue_name("user_123_operations")
    .build();

let spec2 = JobSpecBuilder::new()
    .queue_name("user_123_operations")
    .build();

utils.add_job(UpdateProfile { /* ... */ }, spec1).await?;
utils.add_job(SendEmail { /* ... */ }, spec2).await?;
```

### Cron Jobs

You can schedule recurring jobs with the typed cron API:

```rust,ignore
use graphile_worker::{Cron, CrontabFill, WorkerOptions};

// Run a task daily at 8:00 AM UTC, with one hour of backfill.
let worker = WorkerOptions::default()
    .define_job::<SendDailyReport>()
    .with_cron(
        Cron::daily_at::<SendDailyReport>(8, 0)?
            .fill(CrontabFill::hours(1)),
    )
    // ... other configuration
    .init()
    .await?;
```

Crontab text is still supported when you need Graphile Worker's file-style syntax:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendDailyReport>()
    .with_cron("0 8 * * * send_daily_report")?
    // ... other configuration
    .init()
    .await?;
```

`with_crontab(...)` remains available as a deprecated compatibility alias.

### Local Queue

The Local Queue feature batch-fetches jobs from the database and caches them locally, significantly reducing database load in high-throughput scenarios.

```rust,ignore
use graphile_worker::{WorkerOptions, LocalQueueConfig, RefetchDelayConfig};
use std::time::Duration;

let worker = WorkerOptions::default()
    .local_queue(
        LocalQueueConfig::default()
            .with_size(100)                              // Cache up to 100 jobs
            .with_ttl(Duration::from_secs(300))          // Return unclaimed jobs after 5 minutes
            .with_refetch_delay(
                RefetchDelayConfig::default()
                    .with_duration(Duration::from_millis(100))  // Delay between refetches
                    .with_threshold(10)                         // Refetch when queue drops below 10
            )
    )
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

The Local Queue operates in several modes:
- **Polling**: Actively fetching jobs from the database
- **Waiting**: Jobs are cached locally, serving from cache
- **TtlExpired**: Cache TTL expired, returning jobs to database

Key benefits:
- Reduces database round-trips by fetching jobs in batches
- Configurable cache size and TTL
- Automatic return of unclaimed jobs on shutdown or TTL expiry
- Refetch delay prevents thundering herd on empty queues

### Lifecycle Hooks

You can observe and intercept job lifecycle events using plugins that implement the `Plugin` trait. This is useful for logging, metrics, validation, and custom job handling logic.

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use graphile_worker::{
    Plugin, HookRegistry, HookResult,
    WorkerStart, JobStart, JobComplete, JobFail, BeforeJobRun,
};

struct MetricsPlugin {
    jobs_started: AtomicU64,
    jobs_completed: AtomicU64,
}

impl Plugin for MetricsPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            println!("Worker {} started", ctx.worker_id);
        });

        let jobs_started = Arc::new(self.jobs_started);
        let jobs_completed = Arc::new(self.jobs_completed);

        {
            let jobs_started = jobs_started.clone();
            hooks.on(JobStart, move |ctx| {
                let jobs_started = jobs_started.clone();
                async move {
                    jobs_started.fetch_add(1, Ordering::Relaxed);
                    println!("Job {} started", ctx.job.id());
                }
            });
        }

        {
            let jobs_completed = jobs_completed.clone();
            hooks.on(JobComplete, move |ctx| {
                let jobs_completed = jobs_completed.clone();
                async move {
                    jobs_completed.fetch_add(1, Ordering::Relaxed);
                    println!("Job {} completed in {:?}", ctx.job.id(), ctx.duration);
                }
            });
        }

        hooks.on(JobFail, async |ctx| {
            println!("Job {} failed: {}", ctx.job.id(), ctx.error);
        });
    }
}
```

#### Intercepting Jobs

The `BeforeJobRun` and `AfterJobRun` hooks can intercept jobs and change their behavior:

```rust,ignore
struct ValidationPlugin;

impl Plugin for ValidationPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobRun, async |ctx| {
            // Skip jobs with a "skip" flag in their payload
            if ctx.payload.get("skip").and_then(|v| v.as_bool()).unwrap_or(false) {
                return HookResult::Skip;
            }

            // Fail jobs with invalid data
            if ctx.payload.get("invalid").is_some() {
                return HookResult::Fail("Invalid payload".into());
            }

            // Continue with normal execution
            HookResult::Continue
        });
    }
}
```

#### Registering Plugins

Add plugins when configuring the worker:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendEmail>()
    .add_plugin(MetricsPlugin::new())
    .add_plugin(ValidationPlugin)
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Multiple plugins can be registered and they will all receive hook calls in the order they were added.

#### Available Hooks

| Hook | Type | Description |
|------|------|-------------|
| `WorkerInit` | Observer | Called when worker is initializing |
| `WorkerStart` | Observer | Called when worker starts processing |
| `WorkerShutdown` | Observer | Called when worker is shutting down |
| `JobFetch` | Observer | Called when a job is fetched from the queue |
| `JobStart` | Observer | Called before a job starts executing |
| `JobComplete` | Observer | Called after a job completes successfully |
| `JobFail` | Observer | Called when a job fails (will retry) |
| `JobPermanentlyFail` | Observer | Called when a job exceeds max attempts |
| `JobInterrupted` | Observer | Called when recovery interrupts a job |
| `WorkerRecovered` | Observer | Called after jobs are recovered from inactive workers |
| `CronTick` | Observer | Called on each cron scheduler tick |
| `CronJobScheduled` | Observer | Called when a cron job is scheduled |
| `LocalQueueInit` | Observer | Called when local queue is initialized |
| `LocalQueueSetMode` | Observer | Called when local queue changes mode |
| `LocalQueueGetJobsComplete` | Observer | Called after batch fetching jobs |
| `LocalQueueReturnJobs` | Observer | Called when jobs are returned to database |
| `LocalQueueRefetchDelayStart` | Observer | Called when refetch delay starts |
| `LocalQueueRefetchDelayAbort` | Observer | Called when refetch delay is aborted |
| `LocalQueueRefetchDelayExpired` | Observer | Called when refetch delay expires |
| `BeforeJobRun` | Interceptor | Can skip, fail, or continue job execution |
| `AfterJobRun` | Interceptor | Can modify the job result after execution |
| `BeforeJobSchedule` | Interceptor | Can skip, fail, or transform job before scheduling |
| `JobRecovery` | Interceptor | Can customize recovery for interrupted jobs |

## Job Management Utilities

The `WorkerUtils` class provides methods for managing jobs:

```rust,ignore
// Get a WorkerUtils instance
let utils = worker.create_utils();

// Remove a job by its key
utils.remove_job("job_key_123").await?;

// Mark jobs as completed
utils.complete_jobs(&[job_id1, job_id2]).await?;

// Permanently fail jobs with a reason
utils.permanently_fail_jobs(&[job_id3, job_id4], "Invalid data").await?;

// Reschedule jobs
let options = RescheduleJobOptions {
    run_at: Some(Utc::now() + chrono::Duration::minutes(60)),
    priority: Some(5),
    max_attempts: Some(3),
    ..Default::default()
};
utils.reschedule_jobs(&[job_id5, job_id6], options).await?;

// Run database cleanup tasks
utils.cleanup(&[
    CleanupTask::DeletePermanentlyFailedJobs,
    CleanupTask::GcTaskIdentifiers,
    CleanupTask::GcJobQueues,
]).await?;

// List heartbeat-registered workers and mark stale entries
let active_workers = utils
    .list_active_workers(Duration::from_secs(300))
    .await?;

// Recover jobs from stale workers and orphan locks
let result = utils
    .sweep_stale_workers(SweepStaleWorkersOptions {
        sweep_threshold: Some(Duration::from_secs(300)),
        recovery_delay: Some(Duration::from_secs(30)),
        dry_run: false,
        ..Default::default()
    })
    .await?;
```

## CLI

The workspace includes a dedicated `graphile_worker_cli` crate with a
`graphile-worker` binary for managing jobs from a terminal.

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

Use `--schema` or `GRAPHILE_WORKER_SCHEMA` for non-default schemas, and
`--json` when machine-readable output is preferred.

The CLI can also serve the embedded Leptos admin UI. It serves its HTML, API,
Leptos/WASM client bundle, Tailwind CSS, and icon assets from the same binary.
HTTP Basic auth is the default; when no password is configured, the CLI prints a
random password at startup.

The crate ships prebuilt admin UI assets, so `cargo install graphile_worker_cli`
does not require npm, `wasm-bindgen`, or the `wasm32-unknown-unknown` Rust
target. Maintainers can force a source rebuild with
`GRAPHILE_WORKER_ADMIN_UI_REBUILD=1`; that path needs those tools. Use
`GRAPHILE_WORKER_ADMIN_UI_UPDATE_PREBUILT=1` when intentionally refreshing the
checked-in prebuilt assets.

```bash
DATABASE_URL=postgres://postgres:postgres@localhost/postgres graphile-worker admin
graphile-worker admin --listen 127.0.0.1:8080 --read-only
graphile-worker admin --auth bearer
graphile-worker admin --auth header --header-name x-admin-token
```

The admin UI supports dashboard stats, job list filtering, pasted multi-value
column filters, row selection, copy JSON/CSV actions, job add/complete/fail/run
now/reschedule/remove-by-key actions, queue and worker views, active worker
heartbeats, force unlock, stale worker recovery, cleanup, migrations, theming,
and dark mode.

## Feature List

- **Flexible deployment**: Run standalone or embedded in your application
- **Multi-language support**: Use from Rust, SQL or alongside the Node.js version
- **Performance optimized**:
  - Low latency job execution (typically under 3ms)
  - PostgreSQL `LISTEN`/`NOTIFY` for immediate job notifications
  - `SKIP LOCKED` for efficient job fetching
- **Robust job processing**:
  - Parallel processing with customizable concurrency
  - Serialized execution via named queues
  - Automatic retries with exponential backoff
  - Customizable retry counts (default: 25 attempts over ~3 days)
  - Opt-in worker heartbeat recovery for crashed workers and orphan locks
- **Scheduling features**:
  - Delayed execution with `run_at`
  - Job prioritization
  - Crontab-like recurring tasks
  - Task deduplication via `job_key`
  - Batch job processing with partial retry support
- **Lifecycle hooks**: Observe and intercept job events for logging, metrics, and validation
- **Type safety**: End-to-end type checking of job payloads
- **Minimal overhead**: Direct serialization of task payloads

## Requirements

- **PostgreSQL 12+**
  - Required for the `generated always as (expression)` feature
  - May work with older versions but has not been tested

## Project Status

Production ready but the API may continue to evolve. If you encounter any issues or have feature requests, please open an issue on GitHub.

## Acknowledgments

This library is a Rust port of the excellent [Graphile Worker](https://github.com/graphile/worker) by [Benjie Gillam](https://github.com/benjie). If you find this library useful, please consider [sponsoring Benjie's work](https://github.com/sponsors/benjie), as all the research and architecture design was done by him.

## License

MIT License - See [LICENSE.md](LICENSE.md)
