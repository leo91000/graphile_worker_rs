# Getting Started

Graphile Worker RS is a PostgreSQL-backed job queue for Rust applications. Use
it when application work should move out of the request path: sending email,
generating files, running calculations, scheduling follow-up work, or processing
jobs created by PostgreSQL triggers and functions.

This section is an adoption roadmap. It points to the pages that help you move
from deciding whether Graphile Worker RS fits your application to running your
first worker and exploring the examples.

## Adoption Roadmap

1. Decide whether a PostgreSQL-backed queue is the right model for your work.
   Start with [When to Use Graphile Worker RS](when-to-use.md).
2. Add the crate and choose the runtime, TLS, and database driver features your
   application will use. See [Installation](installation.md).
3. Run a minimal worker to prove your database connection, migrations, task
   registration, and job execution flow. Follow [Quick Start](quick-start.md).
4. Turn that minimal worker into an application shape you can keep: define task
   payloads, register handlers, provide a PostgreSQL pool, and decide how the
   worker should run. See [First Worker](first-worker.md).
5. Compare the repository examples with your use case. The
   [Examples Tour](examples.md) is the best next stop once the basic worker is
   running.

## What You Build First

A worker needs three application-level pieces:

- A serializable payload type.
- A `TaskHandler` implementation with a stable `IDENTIFIER`.
- `WorkerOptions` configured with a PostgreSQL pool and registered handlers.

The shape is visible in `examples/simple.rs`:

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, WorkerContext};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}

impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Hello {} !", self.message);
        Ok(())
    }
}
```

The worker is then configured with a connection pool, concurrency, schema, and
registered task:

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .concurrency(2)
    .schema("example_simple_worker")
    .define_job::<SayHello>()
    .pg_pool(pg_pool)
    .init()
    .await?;

worker.run().await?;
```

## Adding Work to the Queue

After initialization, create utilities from the worker and add jobs with a
payload and `JobSpecBuilder`. The simple example schedules a job for later:

```rust,ignore
use chrono::{offset::Utc, Duration};
use graphile_worker::JobSpecBuilder;

let helpers = worker.create_utils();

helpers
    .add_job(
        SayHello {
            message: "world".to_string(),
        },
        JobSpecBuilder::new()
            .run_at(Utc::now() + Duration::seconds(10))
            .build(),
    )
    .await?;
```

That flow is enough to validate the core model: enqueue typed work, store it in
PostgreSQL, and let the worker execute the matching handler.

## Installation Choices

The default crate features enable Tokio, Rustls TLS, and the SQLx driver. A
basic dependency setup looks like this:

```toml
[dependencies]
graphile_worker = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The crate also exposes feature flags for `runtime-async-std`,
`tls-native-tls`, `driver-tokio-postgres`, and OpenTelemetry compatibility.
Use [Installation](installation.md) first, then check
[Runtime, TLS, and Drivers](../configuration/runtime-drivers.md) and
[Feature Flags](../reference/features.md) when your application needs a
non-default combination.

## Where to Go Next

- [When to Use Graphile Worker RS](when-to-use.md): confirm the queue model and
  tradeoffs.
- [Installation](installation.md): add the crate and choose features.
- [Quick Start](quick-start.md): run the smallest useful worker.
- [First Worker](first-worker.md): structure your first real task handler.
- [Examples Tour](examples.md): map repository examples to common use cases.
- [Core Concepts](../concepts/index.md): learn how tasks, workers, scheduling,
  queues, and the database schema fit together.
- [Configuration](../configuration/index.md): tune worker options, shutdown,
  runtime choices, and application state.
- [Operations](../operations/index.md): prepare migrations, deployment,
  observability, CLI usage, and admin tooling.
