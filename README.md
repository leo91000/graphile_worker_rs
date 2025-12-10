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

- No support for batch jobs yet (create an issue if you need this feature)
- In the Node.js version, each process has its own worker_id. In the Rust version, there is only one worker_id, and jobs are processed in your async runtime thread

## Installation

Add the library to your project:

```bash
cargo add graphile_worker
```

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

Set up the worker with your configuration options and run it:

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

#### Custom shutdown handling

Graphile Worker installs OS-level signal handlers (like `SIGINT`/`SIGTERM`) so
it can shut down gracefully when you press Ctrl+C. If your application already
owns the shutdown lifecycle, disable the built-in listeners and call
`Worker::request_shutdown()` when your orchestrator asks the worker to stop:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .listen_os_shutdown_signals(false) // prevent installing Ctrl+C handlers
    // ... other configuration
    .init()
    .await?;

tokio::pin! {
    let run_loop = worker.run();
}

tokio::select! {
    // Main worker loop
    result = &mut run_loop => result?,
    // Notify the worker when the host framework wants to stop
    () = on_shutdown() => {
        worker.request_shutdown();
        (&mut run_loop).await; // drain gracefully before returning
    }
}
```

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

You can schedule recurring jobs using crontab syntax:

```rust,ignore
// Run a task daily at 8:00 AM
let worker = WorkerOptions::default()
    .with_crontab("0 8 * * * send_daily_report")?
    .define_job::<SendDailyReport>()
    // ... other configuration
    .init()
    .await?;
```

### Lifecycle Hooks

You can observe and intercept job lifecycle events using plugins that implement the `LifecycleHooks` trait. This is useful for logging, metrics, validation, and custom job handling logic.

```rust,ignore
use std::sync::atomic::{AtomicU64, Ordering};
use graphile_worker::{
    LifecycleHooks, HookResult, JobStartContext, JobCompleteContext,
    JobFailContext, BeforeJobRunContext, WorkerStartContext,
};

struct MetricsPlugin {
    jobs_started: AtomicU64,
    jobs_completed: AtomicU64,
}

impl LifecycleHooks for MetricsPlugin {
    async fn on_worker_start(&self, ctx: WorkerStartContext) {
        println!("Worker {} started", ctx.worker_id);
    }

    async fn on_job_start(&self, ctx: JobStartContext) {
        self.jobs_started.fetch_add(1, Ordering::Relaxed);
        println!("Job {} started", ctx.job.id());
    }

    async fn on_job_complete(&self, ctx: JobCompleteContext) {
        self.jobs_completed.fetch_add(1, Ordering::Relaxed);
        println!("Job {} completed in {:?}", ctx.job.id(), ctx.duration);
    }

    async fn on_job_fail(&self, ctx: JobFailContext) {
        println!("Job {} failed: {}", ctx.job.id(), ctx.error);
    }
}
```

#### Intercepting Jobs

The `before_job_run` and `after_job_run` hooks can intercept jobs and change their behavior:

```rust,ignore
struct ValidationPlugin;

impl LifecycleHooks for ValidationPlugin {
    async fn before_job_run(&self, ctx: BeforeJobRunContext) -> HookResult {
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
| `on_worker_init` | Observer | Called when worker is initializing |
| `on_worker_start` | Observer | Called when worker starts processing |
| `on_worker_shutdown` | Observer | Called when worker is shutting down |
| `on_job_fetch` | Observer | Called when a job is fetched from the queue |
| `on_job_start` | Observer | Called before a job starts executing |
| `on_job_complete` | Observer | Called after a job completes successfully |
| `on_job_fail` | Observer | Called when a job fails (will retry) |
| `on_job_permanently_fail` | Observer | Called when a job exceeds max attempts |
| `on_cron_tick` | Observer | Called on each cron scheduler tick |
| `on_cron_job_scheduled` | Observer | Called when a cron job is scheduled |
| `before_job_run` | Interceptor | Can skip, fail, or continue job execution |
| `after_job_run` | Interceptor | Can modify the job result after execution |

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
    CleanupTask::DeletePermenantlyFailedJobs,
    CleanupTask::GcTaskIdentifiers,
    CleanupTask::GcJobQueues,
]).await?;
```

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
- **Scheduling features**:
  - Delayed execution with `run_at`
  - Job prioritization
  - Crontab-like recurring tasks
  - Task deduplication via `job_key`
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

