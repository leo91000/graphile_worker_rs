# Quick Start

This page shows the smallest useful Graphile Worker RS setup: define one typed
task, register it with a worker, add a job, and start processing jobs.

## Add Dependencies

Graphile Worker RS runs against PostgreSQL. With the default Tokio runtime,
rustls TLS, and SQLx driver, a minimal application can connect with a database
URL and these dependencies:

```toml
[dependencies]
graphile_worker = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
```

## Define a Typed Task

A task is a Rust type that can be serialized into the job payload and
deserialized when the worker runs it. Implement `TaskHandler` for the payload
type and give it a stable identifier.

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
    body: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending '{}' to {}", self.subject, self.to);
        Ok::<(), String>(())
    }
}
```

The identifier is the task name stored in PostgreSQL. Jobs with this identifier
will be decoded as `SendEmail` and passed to `SendEmail::run`.

## Create and Run a Worker

Configure a PostgreSQL connection string, register the task with `define_job`,
initialize the worker, and call `run`.

```rust,ignore
use graphile_worker::WorkerOptions;

async fn run_worker() -> Result<(), Box<dyn std::error::Error>> {
    let worker = WorkerOptions::default()
        .database_url("postgres://postgres:password@localhost/mydb")
        .concurrency(5)
        .schema("graphile_worker")
        .define_job::<SendEmail>()
        .init()
        .await?;

    worker.run().await?;

    Ok(())
}
```

`init` connects to PostgreSQL, runs the worker migrations for the configured
schema, registers the task handlers, and returns a `Worker`. The schema defaults
to `graphile_worker` if you do not set one.

## Add a Job

After initialization, use `create_utils` to get `WorkerUtils`. Its `add_job`
method accepts the typed task payload and a `JobSpec`.

```rust,ignore
use graphile_worker::JobSpecBuilder;

let helpers = worker.create_utils();

helpers
    .add_job(
        SendEmail {
            to: "ada@example.com".to_string(),
            subject: "Welcome".to_string(),
            body: "Thanks for signing up.".to_string(),
        },
        JobSpecBuilder::new().build(),
    )
    .await?;
```

For scheduled jobs, set `run_at` on the job spec before building it. This
example uses `chrono`; add it to your application if you schedule from Rust
timestamps:

```rust,ignore
use chrono::{Duration, offset::Utc};
use graphile_worker::JobSpecBuilder;

helpers
    .add_job(
        SendEmail {
            to: "ada@example.com".to_string(),
            subject: "Welcome".to_string(),
            body: "Thanks for signing up.".to_string(),
        },
        JobSpecBuilder::new()
            .run_at(Utc::now() + Duration::seconds(10))
            .build(),
    )
    .await?;
```

## Complete Shape

This combines the pieces into one application shape. In a real application you
can add jobs from request handlers, services, or startup code while one or more
workers process them.

```rust,ignore
use graphile_worker::{
    IntoTaskHandlerResult, JobSpecBuilder, TaskHandler, WorkerContext, WorkerOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
    body: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending '{}' to {}", self.subject, self.to);
        Ok::<(), String>(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = WorkerOptions::default()
        .database_url("postgres://postgres:password@localhost/mydb")
        .concurrency(5)
        .schema("graphile_worker")
        .define_job::<SendEmail>()
        .init()
        .await?;

    worker
        .create_utils()
        .add_job(
            SendEmail {
                to: "ada@example.com".to_string(),
                subject: "Welcome".to_string(),
                body: "Thanks for signing up.".to_string(),
            },
            JobSpecBuilder::new().build(),
        )
        .await?;

    worker.run().await?;

    Ok(())
}
```

For a runnable example in the repository, see `examples/simple.rs`. For more
configuration options, continue with [Configuration](../configuration/index.md).
