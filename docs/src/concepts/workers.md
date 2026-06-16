# Worker Lifecycle

A worker is built with `WorkerOptions`, initialized with `init()`, and then
started with either `run()` or `run_once()`. The builder collects configuration;
`init()` turns that configuration into a ready `Worker`.

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(5)
    .schema("graphile_worker")
    .database_url("postgres://postgres:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?;

worker.run().await?;
```

`Worker::options()` is an equivalent entry point if you prefer to start from the
worker type:

```rust,ignore
let worker = graphile_worker::Worker::options()
    .database_url("postgres://postgres:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?;
```

## Initialization

`WorkerOptions::init()` performs the setup work that must happen before jobs can
be processed:

- uses the supplied database connection, or creates one from `database_url`
- runs the database migrations for the configured schema
- registers the configured task identifiers in the database
- creates a random worker id with the `graphile_worker_` prefix
- applies defaults such as a one second poll interval and CPU-count concurrency
- prepares shutdown, recovery, hook, queue, and batcher state

`init()` returns an error if the worker cannot connect to the database, no
database URL or pool was supplied, migrations fail, task registration fails, or
local queue configuration is invalid.

## Database Configuration

A worker needs a PostgreSQL database before `init()` can succeed. You can pass a
URL and let Graphile Worker RS create the pool:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .database_url("postgres://postgres:password@localhost/mydb")
    .max_pg_conn(20)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

Or you can pass an existing database connection. With the SQLx driver, `pg_pool`
is a convenience wrapper:

```rust,ignore
let pg_pool = sqlx::postgres::PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://postgres:password@localhost/mydb")
    .await?;

let worker = graphile_worker::WorkerOptions::default()
    .pg_pool(pg_pool)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

If both an existing database connection and a URL are provided, the existing
connection is used. `max_pg_conn` only applies when the worker creates a pool
from `database_url`.

The schema defaults to `graphile_worker`. Set a custom schema when you want to
isolate worker tables:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .schema("my_app_worker")
    .database_url("postgres://postgres:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?;
```

## Task Registration

Workers only run tasks registered on the builder. The common path is
`define_job::<T>()`, where `T` implements `TaskHandler`:

```rust,ignore
use graphile_worker::{
    IntoTaskHandlerResult, TaskHandler, WorkerContext, WorkerOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending email to {}", self.to);
    }
}

let worker = WorkerOptions::default()
    .define_job::<SendEmail>()
    .database_url("postgres://postgres:password@localhost/mydb")
    .init()
    .await?;
```

For reusable modules, collect definitions and register them with `define_jobs`.
Batch handlers can be registered with `define_batch_job`.

Jobs with forbidden flags are skipped by that worker:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .add_forbidden_flag("high_memory")
    .define_job::<SendEmail>()
    .database_url("postgres://postgres:password@localhost/mydb")
    .init()
    .await?;
```

## Concurrency and Polling

`concurrency` controls the maximum number of jobs the worker processes at the
same time. If it is not set, the default is the number of logical CPUs. Passing
`0` panics.

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(10)
    .poll_interval(std::time::Duration::from_millis(500))
    .database_url("postgres://postgres:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?;
```

`poll_interval` controls how often the worker checks PostgreSQL for work when
notification delivery is not enough, or when jobs are scheduled for the future.
The default is one second.

## Running Continuously

`Worker::run()` is the normal long-running mode for application workers. It:

- registers the worker for recovery bookkeeping
- emits the worker start hook
- starts the crontab scheduler and the job runner together
- runs until its shutdown signal resolves or an error stops the runner
- waits for completion and failure batchers to shut down
- stops recovery tasks
- emits the worker shutdown hook
- deregisters the worker

```rust,ignore
graphile_worker::WorkerOptions::default()
    .concurrency(5)
    .database_url("postgres://postgres:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?
    .run()
    .await?;
```

Use `run()` for service processes, web application sidecars, and deployments
where the worker should keep listening for new jobs.

## Running Once

`Worker::run_once()` processes currently available jobs and then returns. It
uses the same configured concurrency and task handlers, but it does not start
the continuous lifecycle used by `run()`.

This is useful for scripts, tests, local maintenance commands, and one-shot
workers:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(2)
    .schema("example_simple_worker")
    .define_job::<SayHello>()
    .pg_pool(pg_pool)
    .init()
    .await?;

let helpers = worker.create_utils();

for i in 0..10 {
    helpers
        .add_job(
            SayHello {
                message: format!("world {}", i),
            },
            graphile_worker::JobSpec::default(),
        )
        .await?;
}

worker.run_once().await?;
```

When a run-once job belongs to a queue, the worker checks for another job after
releasing it so queued work can continue in order.

## Shutdown

Each worker has an internal shutdown signal. By default, Graphile Worker RS also
listens for OS shutdown signals such as `SIGINT` and `SIGTERM`. You can request
shutdown from application code:

```rust,ignore
worker.request_shutdown();
```

If your host application already owns shutdown handling, pass a custom signal
and disable the built-in OS listeners:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .listen_os_shutdown_signals(false)
    .shutdown_signal(async {
        wait_for_application_shutdown().await;
    })
    .shutdown_grace_period(std::time::Duration::from_secs(10))
    .shutdown_interrupted_job_retry_delay(std::time::Duration::from_secs(30))
    .database_url("postgres://postgres:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?;

worker.run().await?;
```

The same settings can be supplied as a `WorkerShutdownConfig` with
`worker_shutdown(config)`.

Dropping a `Worker` also notifies the internal shutdown signal as a safety net,
but normal applications should shut down explicitly through the configured
signal or `request_shutdown()`.

## Worker IDs and the Node.js Version

Graphile Worker RS is compatible with the original Node.js Graphile Worker for
the queue schema, but worker identity is different. In the Node.js version, each
process has its own `worker_id`. In the Rust version, `init()` creates one
worker id for the `Worker` instance, and jobs are processed concurrently by the
enabled async runtime using that same worker id.
