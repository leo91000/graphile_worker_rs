# First Worker

This page walks through the smallest useful Graphile Worker RS flow:

1. Create a PostgreSQL pool.
2. Define a task handler.
3. Configure `WorkerOptions`.
4. Initialize the worker.
5. Add a job.
6. Run the worker.
7. Let shutdown happen gracefully.

The examples use Tokio and SQLx, matching the default setup used by the
repository examples.

## Define a Handler

A worker processes jobs by matching each job's task identifier to a registered
`TaskHandler`. The handler type is also the payload type, so it should derive
`Serialize` and `Deserialize`.

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}

impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Hello {} !", self.message);
        Ok::<(), String>(())
    }
}
```

`IDENTIFIER` is the task name stored in PostgreSQL. Register the same handler
type on any worker that should be able to process jobs with that identifier.

## Create the Pool

You can pass an existing SQLx PostgreSQL pool with `pg_pool`. This is useful
when your application already owns database configuration.

```rust,ignore
use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

let pg_options = PgConnectOptions::from_str(
    "postgres://postgres:root@localhost:5432",
)?;

let pg_pool = PgPoolOptions::new()
    .max_connections(5)
    .connect_with(pg_options)
    .await?;
```

You can also let Graphile Worker create the pool from a URL with
`database_url`. When using `database_url`, `max_pg_conn` controls the pool size
and defaults to 20 if it is not set.

```rust,ignore
let options = graphile_worker::WorkerOptions::default()
    .database_url("postgres://postgres:root@localhost:5432")
    .max_pg_conn(5);
```

If both an existing database connection and `database_url` are provided, the
existing connection takes precedence.

## Configure and Initialize

`WorkerOptions` is a builder. The usual first-worker options are:

- `concurrency`: maximum jobs processed at the same time. If omitted, it
  defaults to the number of logical CPUs.
- `schema`: PostgreSQL schema used for Graphile Worker tables. If omitted, it
  defaults to `graphile_worker`.
- `define_job::<T>()`: registers a `TaskHandler` type.
- `pg_pool`, `database`, or `database_url`: provides the database connection.

Calling `init().await` connects to the database if needed, runs migrations for
the selected schema, registers task details, creates the worker id, and returns
a ready `Worker`.

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .concurrency(2)
    .schema("example_simple_worker")
    .define_job::<SayHello>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

## Add a Job

After initialization, create utilities from the worker and enqueue a job.
`JobSpec::default()` schedules it with default job options.

```rust,ignore
use graphile_worker::JobSpec;

let helpers = worker.create_utils();

helpers
    .add_job(
        SayHello {
            message: "world".to_string(),
        },
        JobSpec::default(),
    )
    .await?;
```

To schedule a job for later, build a `JobSpec` with `JobSpecBuilder`.

```rust,ignore
use chrono::{Duration, Utc};
use graphile_worker::JobSpecBuilder;

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

## Run the Worker

`run()` starts the long-running worker loop. It registers the worker, starts job
sources, processes jobs until shutdown is requested, waits for batchers to
finish, emits shutdown hooks, and deregisters the worker.

```rust,ignore
worker.run().await?;
```

For scripts and smoke tests, `run_once()` processes currently available jobs and
then returns.

```rust,ignore
worker.run_once().await?;
```

## Full Example

```rust,ignore
use std::str::FromStr;

use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext, WorkerOptions,
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}

impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Hello {} !", self.message);
        Ok::<(), String>(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pg_options = PgConnectOptions::from_str(
        "postgres://postgres:root@localhost:5432",
    )?;

    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(pg_options)
        .await?;

    let worker = WorkerOptions::default()
        .concurrency(2)
        .schema("example_simple_worker")
        .define_job::<SayHello>()
        .pg_pool(pg_pool)
        .init()
        .await?;

    let helpers = worker.create_utils();

    helpers
        .add_job(
            SayHello {
                message: "world".to_string(),
            },
            JobSpec::default(),
        )
        .await?;

    worker.run().await?;

    Ok(())
}
```

## Shutdown

By default, the worker listens for OS shutdown signals such as Ctrl+C and
SIGTERM. When a shutdown signal arrives, in-flight jobs are given a grace
period to finish. The default grace period is 5 seconds, and shutdown-aborted
jobs are retried after a default delay of 30 seconds.

If the host application already owns shutdown handling, pass a custom shutdown
future and disable OS signal listeners:

```rust,ignore
use graphile_worker::WorkerShutdownConfig;
use std::time::Duration;

let shutdown = WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(false)
    .grace_period(Duration::from_secs(10))
    .interrupted_job_retry_delay(Duration::from_secs(30))
    .shutdown_signal(async {
        // Complete this future when your application wants the worker to stop.
    });

let worker = WorkerOptions::default()
    .worker_shutdown(shutdown)
    .define_job::<SayHello>()
    .pg_pool(pg_pool)
    .init()
    .await?;

worker.run().await?;
```

The same settings can also be configured directly on `WorkerOptions` with
`listen_os_shutdown_signals`, `shutdown_signal`, `shutdown_grace_period`, and
`shutdown_interrupted_job_retry_delay`.

Next, see [Scheduling Jobs](../guides/scheduling-jobs.md) for job timing options and
[Worker Options](../configuration/worker-options.md) for more configuration
details.
