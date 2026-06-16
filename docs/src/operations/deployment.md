# Deployment

Graphile Worker RS runs as an application process backed by PostgreSQL. A
production deployment should treat the worker like any other stateful database
client: size the PostgreSQL pool deliberately, run schema migrations before work
starts, handle shutdown signals, and expose enough logs or hooks to understand
job flow.

## Database and Migrations

Each worker needs a PostgreSQL connection, either from an existing database pool
or from a connection URL:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .database_url(&std::env::var("DATABASE_URL")?)
    .schema("graphile_worker")
    .define_job::<SendEmail>()
    .init()
    .await?;
```

If you already manage your own pool, pass it to the worker instead:

```rust,ignore
let pg_pool = sqlx::postgres::PgPoolOptions::new()
    .max_connections(10)
    .connect(&std::env::var("DATABASE_URL")?)
    .await?;

let worker = graphile_worker::WorkerOptions::default()
    .pg_pool(pg_pool)
    .schema("graphile_worker")
    .define_job::<SendEmail>()
    .init()
    .await?;
```

`WorkerOptions::init()` runs the Graphile Worker migrations for the configured
schema before registering task handlers and returning a runnable worker. The
default schema is `graphile_worker`; choose a different schema when you want to
isolate worker tables for an application, environment, or tenant.

The CLI can also run migrations explicitly:

```bash
graphile-worker --database-url "$DATABASE_URL" migrate
```

## Pool and Concurrency

`concurrency` controls how many jobs a worker process can run at the same time.
If you do not set it, the worker defaults to the number of logical CPUs on the
host. Set it explicitly in production so the same artifact behaves consistently
across instance sizes.

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(8)
    .database_url(&database_url)
    .max_pg_conn(20)
    .define_job::<SendEmail>()
    .init()
    .await?;
```

When the worker creates its own pool from `database_url`, `max_pg_conn` sets the
maximum number of database connections. If you pass an existing pool with
`pg_pool` or `database`, the pool's own configuration is used and
`max_pg_conn` is ignored.

Use practical sizing:

- CPU-heavy jobs usually need concurrency near the available CPU capacity.
- I/O-heavy jobs can often run with higher concurrency.
- Total database connections are the sum of all worker replicas plus the rest
  of your application.
- Keep enough pool capacity for polling, job updates, LISTEN/NOTIFY handling,
  recovery, cron scheduling, and any database work done inside job handlers.

The worker polls PostgreSQL when notifications are not enough and for scheduled
jobs. The default poll interval is one second. Lower intervals can improve
responsiveness but increase database load:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .poll_interval(std::time::Duration::from_millis(500))
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

## Worker Process

A typical production process creates the worker, then calls `run()`:

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = graphile_worker::WorkerOptions::default()
        .concurrency(8)
        .database_url(&std::env::var("DATABASE_URL")?)
        .schema("graphile_worker")
        .define_job::<SendEmail>()
        .init()
        .await?;

    worker.run().await?;
    Ok(())
}
```

Register every task this process is allowed to execute with `define_job`,
`define_batch_job`, or `define_jobs`. To split work across specialized worker
deployments, use job flags and `add_forbidden_flag` so a process skips jobs that
do not belong on that deployment:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .add_forbidden_flag("high_memory")
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

Jobs with the same queue name run in series for that queue. Use queue names when
you need per-user, per-account, or per-resource ordering while still allowing
other queues to run concurrently.

## Shutdown Signals

By default, the worker listens for OS shutdown signals such as SIGINT and
SIGTERM. This is the right behavior for most containers and process managers:
send a shutdown signal and give the process enough grace time to finish or
release in-flight work.

Tune graceful shutdown with `WorkerShutdownConfig` or the convenience setters:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .shutdown_grace_period(std::time::Duration::from_secs(30))
    .shutdown_interrupted_job_retry_delay(std::time::Duration::from_secs(30))
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

If your host application already owns signal handling, disable the built-in OS
listeners and pass a future that resolves when shutdown should begin:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .listen_os_shutdown_signals(false)
    .shutdown_signal(on_shutdown())
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

Graphile Worker still handles graceful draining after the shutdown signal is
received.

## Recovery

Worker recovery is opt-in. Enable it when a job may be left locked after a
process crash, network partition, forced abort, or orchestrator timeout. When
enabled, workers heartbeat in PostgreSQL and a sweeper recovers jobs owned by
workers that have stopped heartbeating.

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .heartbeat_interval(std::time::Duration::from_secs(30))
    .sweep_interval(std::time::Duration::from_secs(60))
    .sweep_threshold(std::time::Duration::from_secs(300))
    .recovery_delay(std::time::Duration::from_secs(30))
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

Recovery sweeps use a PostgreSQL advisory lock so only one sweeper performs the
recovery work at a time. Recovered jobs are unlocked, their attempt count is
decremented back, their queue lock is released, and their `run_at` is delayed by
the configured recovery delay.

You can run the same kind of recovery manually from the CLI:

```bash
graphile-worker --database-url "$DATABASE_URL" sweep-stale-workers --dry-run
graphile-worker --database-url "$DATABASE_URL" sweep-stale-workers --sweep-threshold 5m --recovery-delay 30s
```

Use `--dry-run` first when investigating a production incident.

## Cron

Cron entries are configured on the worker. Use the typed cron API when the task
type is available in Rust:

```rust,ignore
use graphile_worker::{Cron, CrontabFill, WorkerOptions};

let worker = WorkerOptions::default()
    .define_job::<SendDailyReport>()
    .with_cron(
        Cron::daily_at::<SendDailyReport>(8, 0)?
            .fill(CrontabFill::hours(1)),
    )
    .database_url(&database_url)
    .init()
    .await?;
```

Crontab text is also supported:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .define_job::<SendDailyReport>()
    .with_cron("0 8 * * * send_daily_report")?
    .database_url(&database_url)
    .init()
    .await?;
```

For recurring jobs, decide which deployment owns each cron definition. Running
the same cron configuration in multiple independent deployments can schedule the
same recurring work more than intended unless the job key and cron settings are
chosen to deduplicate the workload.

## Local Queue

The local queue batch-fetches jobs from PostgreSQL and serves them from a local
cache. This can reduce database round trips for high-throughput deployments, at
the cost of keeping jobs in a process-local cache until they are claimed,
returned, or the cache TTL expires.

```rust,ignore
use graphile_worker::{LocalQueueConfig, RefetchDelayConfig, WorkerOptions};
use std::time::Duration;

let worker = WorkerOptions::default()
    .local_queue(
        LocalQueueConfig::default()
            .with_size(100)
            .with_queue_count(2)
            .with_ttl(Duration::from_secs(300))
            .with_refetch_delay(
                RefetchDelayConfig::default()
                    .with_duration(Duration::from_millis(100))
                    .with_threshold(10)
                    .with_max_abort_threshold(500),
            ),
    )
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

`size` applies per local queue, so total local capacity is
`size * queue_count`. Workers configured with `forbidden_flags` bypass the local
queue and fetch jobs directly from the database. Benchmark realistic jobs before
turning this on for all production workers; the best settings depend on
PostgreSQL latency, pool size, worker concurrency, job duration, and the number
of replicas.

## Observability

Graphile Worker emits useful events through lifecycle hooks and plugins. Use
hooks for metrics, structured logs, tracing spans, validation, or custom
recovery policy.

```rust,ignore
use graphile_worker::{
    HookRegistry, JobComplete, JobFail, JobStart, Plugin, WorkerShutdown, WorkerStart,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct MetricsPlugin {
    jobs_started: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
}

impl Plugin for MetricsPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            tracing::info!(worker_id = %ctx.worker_id, "worker started");
        });

        let jobs_started = Arc::new(self.jobs_started);
        let jobs_completed = Arc::new(self.jobs_completed);
        let jobs_failed = Arc::new(self.jobs_failed);

        {
            let jobs_started = jobs_started.clone();
            hooks.on(JobStart, move |_ctx| {
                let jobs_started = jobs_started.clone();
                async move {
                    jobs_started.fetch_add(1, Ordering::Relaxed);
                }
            });
        }

        {
            let jobs_completed = jobs_completed.clone();
            hooks.on(JobComplete, move |ctx| {
                let jobs_completed = jobs_completed.clone();
                async move {
                    jobs_completed.fetch_add(1, Ordering::Relaxed);
                    tracing::info!(duration_ms = ?ctx.duration, "job completed");
                }
            });
        }

        {
            let jobs_failed = jobs_failed.clone();
            hooks.on(JobFail, move |ctx| {
                let jobs_failed = jobs_failed.clone();
                async move {
                    jobs_failed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(error = %ctx.error, will_retry = ctx.will_retry, "job failed");
                }
            });
        }

        hooks.on(WorkerShutdown, async |ctx| {
            tracing::info!(worker_id = %ctx.worker_id, reason = ?ctx.reason, "worker shutting down");
        });
    }
}
```

Register plugins at startup:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .add_plugin(MetricsPlugin {
        jobs_started: AtomicU64::new(0),
        jobs_completed: AtomicU64::new(0),
        jobs_failed: AtomicU64::new(0),
    })
    .define_job::<SendEmail>()
    .database_url(&database_url)
    .init()
    .await?;
```

For operational inspection, the CLI can list jobs and inspect worker state:

```bash
graphile-worker --database-url "$DATABASE_URL" list --state ready
graphile-worker --database-url "$DATABASE_URL" stats
graphile-worker --database-url "$DATABASE_URL" queues
graphile-worker --database-url "$DATABASE_URL" workers
```

## Deployment Checklist

- Provide `DATABASE_URL` or a configured PostgreSQL pool.
- Pick a schema and run migrations with `init()` or the CLI `migrate` command.
- Set `concurrency` explicitly for predictable behavior across instance sizes.
- Size database pools for all replicas and job-handler database work.
- Ensure SIGTERM reaches the worker process and the orchestrator grace period is
  longer than the worker grace period.
- Enable recovery when forced exits or node failures are part of your failure
  model.
- Assign cron ownership deliberately.
- Turn on local queue only after measuring throughput and latency with realistic
  jobs.
- Add lifecycle hooks or plugins for logs, metrics, and failure visibility.
