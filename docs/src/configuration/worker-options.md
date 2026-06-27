# Worker Options

`WorkerOptions` is the main builder used to configure a worker before calling
`init()`. The builder collects database settings, registered task handlers,
scheduling rules, hooks, and performance options, then `init()` connects to
PostgreSQL, runs migrations, registers task details, and returns a runnable
worker.

Most applications start with `WorkerOptions::default()`, chain the options they
need, then call `init().await?` followed by `run().await?`.

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .concurrency(5)
    .schema("graphile_worker")
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;

worker.run().await?;
```

## Database configuration

Every worker needs a database connection before `init()` can succeed. You can
either pass an existing connection pool or let the worker create one from a
database URL.

```rust,ignore
let worker = WorkerOptions::default()
    .database_url("postgres://user:password@localhost/mydb")
    .max_pg_conn(20)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

When you already have a SQLx pool, pass it directly:

```rust,ignore
let worker = WorkerOptions::default()
    .pg_pool(pg_pool)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

Important database methods:

- `database(value)` uses an existing driver-agnostic database connection.
- `pg_pool(pool)` is the SQLx convenience wrapper for passing an existing
  `sqlx::PgPool`.
- `database_url(url)` stores the PostgreSQL URL used when no existing
  connection was provided.
- `max_pg_conn(count)` sets the maximum pool size when the worker creates a pool
  from `database_url`; the default is `20`.

If both an existing connection and `database_url` are provided, the existing
connection takes precedence.

## Core runtime settings

Use the core options to control where worker tables live and how aggressively
the worker processes jobs.

```rust,ignore
use std::time::Duration;

let worker = WorkerOptions::default()
    .schema("my_app_worker")
    .concurrency(10)
    .poll_interval(Duration::from_millis(500))
    .use_notification_delivery(true)
    .use_local_time(false)
    .pg_pool(pg_pool)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

Important core methods:

- `schema(name)` sets the PostgreSQL schema for Graphile Worker tables. If it is
  not set, the default schema is `graphile_worker`.
- `concurrency(count)` sets the maximum number of jobs processed at the same
  time. If it is not set, the worker uses the number of logical CPUs. Passing
  `0` panics.
- `poll_interval(duration)` controls fallback polling for new or future jobs.
  The default is one second.
- `use_notification_delivery(value)` controls whether the worker listens for
  PostgreSQL `NOTIFY` wakeups. The default is `true`; set it to `false` to use
  polling only. Notifications are treated as coalesced wakeup hints; when all
  workers are already saturated, extra notifications are dropped and the runner
  keeps draining the listener instead of blocking on worker fanout.
- `use_local_time(value)` controls whether timestamps use application time
  (`true`) or PostgreSQL server time (`false`). The default is PostgreSQL server
  time.

PostgreSQL server time is the safer default when multiple worker processes run
against the same database.

## Job registration

Register every task handler that this worker should be able to run. The common
case is one or more `define_job::<T>()` calls:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendEmail>()
    .define_job::<GenerateReport>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

For larger applications, modules can expose reusable job definitions and the
worker can register them together:

```rust,ignore
use graphile_worker::{JobDefinition, TaskHandler};

pub fn jobs() -> [JobDefinition; 2] {
    [
        SendEmail::definition(),
        GenerateReport::definition(),
    ]
}

let worker = WorkerOptions::default()
    .define_jobs(jobs())
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Important job methods:

- `define_job::<T>()` registers a `TaskHandler`.
- `define_batch_job::<T>()` registers a `BatchTaskHandler`.
- `define_jobs(iterable)` registers reusable `JobDefinition` values.
- `add_forbidden_flag(flag)` makes this worker skip jobs with that flag.

When `add_forbidden_flag` is used, local queue configuration is disabled during
initialization.

## Cron schedules

Cron entries are added with `with_cron` or `with_crons`. Typed cron builders are
useful when you want Rust types to define the task and payload:

```rust,ignore
use graphile_worker::{Cron, CronJobKeyMode, CrontabFill, WorkerOptions};

let worker = WorkerOptions::default()
    .define_job::<SayHello>()
    .pg_pool(pg_pool)
    .with_cron(
        Cron::every_n_minutes::<SayHello>(2)?
            .fill(CrontabFill::minutes(10))
            .job_key("say_hello_dedupe")
            .job_key_mode(CronJobKeyMode::PreserveRunAt)
            .payload(SayHello {
                message: "Crontab".to_string(),
            })?,
    )
    .init()
    .await?;
```

Crontab text is also accepted, but because it must be parsed, the call returns a
`Result`:

```rust,ignore
let options = WorkerOptions::default()
    .define_job::<SendDigest>()
    .with_cron("0 8 * * * send_digest")?;
```

Use `with_crons([...])` when you already have multiple typed crontab values to
append.

## Local queue

`local_queue(config)` enables batch-fetching jobs into an in-process local
queue. This can reduce PostgreSQL load for high-throughput workers by fetching
several jobs at once and distributing them locally to the worker's concurrency
slots.

```rust,ignore
use graphile_worker::{LocalQueueConfig, RefetchDelayConfig, WorkerOptions};
use std::time::Duration;

let worker = WorkerOptions::default()
    .concurrency(4)
    .schema("example_local_queue")
    .define_job::<ProcessItem>()
    .pg_pool(pg_pool)
    .local_queue(
        LocalQueueConfig::default()
            .with_size(50)
            .with_ttl(Duration::from_secs(60))
            .with_refetch_delay(
                RefetchDelayConfig::default()
                    .with_duration(Duration::from_millis(100))
                    .with_threshold(5)
                    .with_max_abort_threshold(20),
            ),
    )
    .init()
    .await?;
```

Local queue settings are validated against the configured `poll_interval` during
`init()`. If forbidden flags are configured, the worker ignores the local queue
configuration.

## Hooks, plugins, and extensions

Worker options can also compose application state and hook behavior:

```rust,ignore
let options = WorkerOptions::default()
    .add_extension(app_config)
    .add_plugin(LocalQueueMonitorPlugin::default())
    .on(JobStart, |ctx| async move {
        tracing::info!("job {} started", ctx.job.id());
    });
```

Important composition methods:

- `add_extension(value)` stores typed application state that handlers can read
  from the worker context.
- `on(event, handler)` registers a hook handler directly.
- `add_plugin(plugin)` registers a plugin that can attach several hooks.

Plugins are a good fit when a feature needs to register several related hooks,
such as monitoring local queue events.

## Completion and failure batching

For high-throughput workers, completion and permanent failure updates can be
batched to reduce SQL round trips:

```rust,ignore
use std::time::Duration;

let worker = WorkerOptions::default()
    .complete_job_batch_delay(Duration::from_millis(5))
    .fail_job_batch_delay(Duration::from_millis(5))
    .define_job::<ProcessItem>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

`complete_job_batch_delay(delay)` batches successful completion updates.
`fail_job_batch_delay(delay)` batches permanent failure updates. Retryable
failures are still processed individually so retry backoff timing can be
preserved.

## Composition examples

### Basic worker

This is the smallest common shape: configure the database, register jobs, and
run.

```rust,ignore
let worker = WorkerOptions::default()
    .schema("example_simple_worker")
    .concurrency(2)
    .define_job::<SayHello>()
    .pg_pool(pg_pool)
    .init()
    .await?;

worker.run().await?;
```

### Scheduled worker

Scheduled workers are ordinary workers with cron entries added before
initialization.

```rust,ignore
let worker = WorkerOptions::default()
    .schema("example_simple_worker")
    .concurrency(10)
    .define_job::<SayHello>()
    .define_job::<SayHello2>()
    .pg_pool(pg_pool)
    .with_cron(
        Cron::every_n_minutes::<SayHello>(2)?
            .fill(CrontabFill::minutes(10))
            .payload(SayHello {
                message: "Crontab".to_string(),
            })?,
    )
    .init()
    .await?;
```

### Throughput-oriented worker

For job bursts, combine a tuned concurrency value with local queueing and small
batching delays.

```rust,ignore
use std::time::Duration;

let worker = WorkerOptions::default()
    .schema("jobs")
    .concurrency(16)
    .poll_interval(Duration::from_millis(500))
    .complete_job_batch_delay(Duration::from_millis(5))
    .fail_job_batch_delay(Duration::from_millis(5))
    .local_queue(
        LocalQueueConfig::default()
            .with_size(100)
            .with_ttl(Duration::from_secs(60)),
    )
    .define_jobs(jobs())
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Tune these values with your own workload, PostgreSQL latency, pool size, and job
duration. Higher concurrency is useful only when the database pool and task
workload can support it.
