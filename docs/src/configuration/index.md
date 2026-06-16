# Configuration

Graphile Worker RS is configured through `WorkerOptions`. Start with the few
choices that define the worker's role, then add specialized options only when
the deployment needs them.

Most applications need to answer these questions:

1. Which runtime, TLS backend, and database driver should the crate compile
   with?
2. Which PostgreSQL connection and schema should this worker use?
3. Which task handlers is this process responsible for?
4. How much work should this process run at once?
5. Does the application need cron, lifecycle hooks, application state, local
   queueing, recovery, or custom shutdown behavior?

## Minimal Worker

A worker needs a database connection, at least one task definition for the jobs
it should process, and a call to `init()`.

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .database_url("postgres://user:password@localhost/mydb")
    .define_job::<SendEmail>()
    .init()
    .await?;

worker.run().await?;
```

`init()` establishes or reuses the database connection, runs migrations for the
configured schema, registers task details, creates a worker id, and returns a
configured worker.

## Decision Map

Use this map to find the option family that matches the decision you are making.

| Decision | Use | Notes |
| --- | --- | --- |
| Compile for Tokio or async-std | Cargo features | Default features enable Tokio, rustls, and SQLx. See [Runtime, TLS, and Drivers](runtime-drivers.md). |
| Connect with an existing pool | `database(...)` or `pg_pool(...)` | An explicit database connection takes precedence over `database_url(...)`. |
| Connect from a URL | `database_url(...)` and `max_pg_conn(...)` | `max_pg_conn(...)` applies only when the worker creates its own pool from a URL. |
| Isolate worker tables | `schema(...)` | Defaults to the Graphile Worker schema when not set. |
| Register handlers | `define_job(...)`, `define_batch_job(...)`, `define_jobs(...)` | Use `define_jobs(...)` when a module exposes reusable job definitions. |
| Limit concurrent execution | `concurrency(...)` | Defaults to the number of logical CPUs and must be greater than zero. |
| Tune database polling | `poll_interval(...)` | Defaults to one second. Notifications still provide low-latency wakeups when available. |
| Skip flagged jobs | `add_forbidden_flag(...)` | Workers with forbidden flags bypass local queueing and fetch directly from the database. |
| Add recurring jobs | `with_cron(...)` or `with_crons(...)` | Accepts typed cron values, raw crontabs, or crontab text depending on input. See [Cron Jobs](../guides/cron.md). |
| Share application state | `add_extension(...)` | Extensions are available from task contexts. See [Application State and Extensions](app-state.md). |
| Observe or intercept lifecycle events | `on(...)` or `add_plugin(...)` | See [Lifecycle Hooks](../guides/hooks.md). |
| Improve throughput with local buffering | `local_queue(...)` | Batch-fetches jobs into a local cache. See [Local Queue](../guides/local-queue.md). |
| Batch completion or permanent failure writes | `complete_job_batch_delay(...)`, `fail_job_batch_delay(...)` | Small delays such as 1-5ms are recommended by the API docs. |
| Recover jobs locked by dead workers | `worker_recovery(...)` or recovery convenience setters | Recovery is opt-in. See [Worker Recovery](../guides/recovery.md). |
| Integrate with host shutdown | `worker_shutdown(...)` or shutdown convenience setters | See [Shutdown](shutdown.md). |

For a complete option reference, see [Worker Options](worker-options.md).

## Database Configuration

Pass a database connection directly when your application already owns the pool:

```rust,ignore
let worker = WorkerOptions::default()
    .pg_pool(pg_pool)
    .schema("graphile_worker")
    .define_job::<SendEmail>()
    .init()
    .await?;
```

Or let the worker create its own connection pool from a PostgreSQL URL:

```rust,ignore
let worker = WorkerOptions::default()
    .database_url("postgres://user:password@localhost/mydb")
    .max_pg_conn(20)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

Use `schema(...)` when you want Graphile Worker tables in a specific PostgreSQL
schema, for example to separate environments or independent worker systems in
the same database.

## Runtime And Driver Features

The default crate features enable:

```toml
graphile_worker = { version = "0.13", features = ["tls-rustls"] }
```

That default path uses Tokio, rustls, and the SQLx driver. To use async-std,
disable default features and enable the async-std runtime explicitly:

```toml
graphile_worker = { version = "0.13", default-features = false, features = [
  "runtime-async-std",
  "tls-rustls",
  "driver-sqlx",
] }
```

Feature choices are compile-time configuration, not `WorkerOptions` methods.
See [Runtime, TLS, and Drivers](runtime-drivers.md) and
[Feature Flags](../reference/features.md) before changing them.

## Worker Role

A process can register one task, a batch task, or a set of reusable job
definitions:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendEmail>()
    .define_batch_job::<ImportContacts>()
    .define_jobs(reporting_jobs())
    .add_forbidden_flag("high_memory")
    .init()
    .await?;
```

Use `add_forbidden_flag(...)` for specialized workers that must refuse jobs with
certain flags. This is a filtering rule for the worker process; it does not
register a handler by itself.

## Throughput And Latency

The primary knobs are `concurrency(...)`, `poll_interval(...)`, local queueing,
and optional batched writes.

```rust,ignore
use std::time::Duration;

let worker = WorkerOptions::default()
    .concurrency(10)
    .poll_interval(Duration::from_millis(500))
    .complete_job_batch_delay(Duration::from_millis(5))
    .fail_job_batch_delay(Duration::from_millis(5))
    .define_job::<SendEmail>()
    .init()
    .await?;
```

`concurrency(...)` controls how many jobs can run at the same time.
`poll_interval(...)` controls fallback polling and future scheduled job checks.
For higher-throughput workloads, [Local Queue](../guides/local-queue.md) can
batch-fetch jobs from PostgreSQL and distribute them locally.

## Scheduling

Use `with_cron(...)` for recurring jobs. Text input returns a result because it
can fail to parse:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendDigest>()
    .with_cron("0 8 * * * send_digest")?
    .init()
    .await?;
```

Typed cron builders and raw crontab values can also be added through
`with_cron(...)` or `with_crons(...)`. See [Cron Jobs](../guides/cron.md) for
the scheduling guide and [Scheduling Model](../concepts/scheduling.md) for the
underlying concepts.

## Application Integration

Use extensions for shared application state, hooks for lifecycle behavior, and
shutdown settings when the host application already owns process signals:

```rust,ignore
use std::time::Duration;

let worker = WorkerOptions::default()
    .add_extension(app_config)
    .listen_os_shutdown_signals(false)
    .shutdown_signal(host_shutdown())
    .shutdown_grace_period(Duration::from_secs(5))
    .define_job::<SendEmail>()
    .init()
    .await?;
```

Shutdown configuration controls when the worker starts graceful shutdown, how
long in-flight jobs may continue, and when shutdown-aborted jobs should be made
eligible for retry. See [Shutdown](shutdown.md).

## Recovery

Worker recovery is disabled by default. Enable it when your deployment needs
jobs locked by crashed or unreachable workers to be released automatically:

```rust,ignore
use std::time::Duration;

let worker = WorkerOptions::default()
    .heartbeat_interval(Duration::from_secs(30))
    .sweep_interval(Duration::from_secs(60))
    .sweep_threshold(Duration::from_secs(300))
    .recovery_delay(Duration::from_secs(30))
    .define_job::<SendEmail>()
    .init()
    .await?;
```

The recovery convenience setters enable recovery while setting their specific
interval or delay. Use `worker_recovery(...)` when you want to build and pass a
full recovery configuration object. See [Worker Recovery](../guides/recovery.md).
