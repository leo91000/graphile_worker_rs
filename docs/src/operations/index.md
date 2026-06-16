# Operations

This section covers the surfaces you use after Graphile Worker RS is running in
an application: schema migrations, the command-line tool, the admin UI, worker
shutdown, recovery, and production readiness checks.

Use these pages when you are preparing a deployment or responding to a queue
incident:

- [CLI](./cli.md) for adding, listing, completing, failing, rescheduling,
  removing, cleaning up, unlocking, and inspecting jobs.
- [Admin UI](./admin-ui.md) for browser-based queue inspection and controlled
  maintenance actions.
- [Migrations](./migrations.md) for installing and updating the PostgreSQL
  schema.
- [Deployment](./deployment.md) for running workers outside local development.
- [Observability](./observability.md) for production visibility.

## Operational surfaces

Graphile Worker RS exposes three main operational entry points.

### Worker runtime

The worker process owns job execution. Production configuration normally starts
with a PostgreSQL pool, a schema name, registered task handlers, concurrency,
and shutdown behavior:

```rust,ignore
use graphile_worker::{WorkerOptions, WorkerShutdownConfig};
use std::time::Duration;

let shutdown = WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(true)
    .grace_period(Duration::from_secs(5))
    .interrupted_job_retry_delay(Duration::from_secs(30));

let worker = WorkerOptions::default()
    .concurrency(5)
    .schema("graphile_worker")
    .worker_shutdown(shutdown)
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;

worker.run().await?;
```

If your host application already owns shutdown, disable the built-in OS signal
listeners and pass the host shutdown future to the worker. The worker still
handles graceful draining, local queue release, batcher flushes, and configured
grace-period behavior for in-flight jobs.

### Command-line tool

The installed binary is `graphile-worker`. It connects through
`--database-url` or `DATABASE_URL` and uses the `graphile_worker` schema by
default.

```bash
graphile-worker --database-url postgres://postgres:postgres@localhost/postgres migrate
DATABASE_URL=postgres://postgres:postgres@localhost/postgres graphile-worker add send_email --payload '{"to":"user@example.com"}'
graphile-worker list --state ready
graphile-worker show 123
graphile-worker complete 123 124
graphile-worker fail 125 --reason "invalid payload"
graphile-worker reschedule 126 --run-at 2026-01-02T03:04:05Z
graphile-worker remove cli-job-key
graphile-worker cleanup
graphile-worker force-unlock graphile_worker_deadbeef
graphile-worker stats
graphile-worker queues
graphile-worker workers
```

For stale worker recovery, dry-run first so you can see which workers would be
recovered:

```bash
graphile-worker sweep-stale-workers --dry-run
graphile-worker sweep-stale-workers --sweep-threshold 5m --recovery-delay 30s
```

### Admin UI

The native admin UI is an Axum server. It serves the browser UI and protected
API routes for:

- session metadata
- queue overview, stats, queues, locked workers, and active workers
- job listing and single-job inspection
- adding raw jobs
- completing, permanently failing, running now, and rescheduling jobs
- removing a job by key
- maintenance actions: migrate, cleanup, force unlock, and sweep stale workers

When embedding the admin UI directly, `AdminServerConfig` defaults to
`127.0.0.1:4000`, uses the `graphile_worker` schema, enables write actions, and
uses Basic auth with a random password for the `admin` user. The
`graphile-worker admin` CLI command overrides the listen default to
`127.0.0.1:5678`. Authentication modes available in the server configuration are
Basic, Bearer token, custom header token, and no auth. No-auth mode is rejected
on non-loopback listen addresses.

Run write-capable admin UI instances behind an access-controlled boundary. The
server adds no-store cache headers for dynamic routes, caches static assets for
one hour, sets `X-Content-Type-Options: nosniff`, denies framing, uses
`Referrer-Policy: no-referrer`, and applies a Content Security Policy. API
write methods also require the admin CSRF token.

Use read-only mode for shared inspection surfaces. In read-only mode, write
routes return `403` instead of mutating jobs or running maintenance.

## Migrations

Migrations install and update the Graphile Worker PostgreSQL schema. The
migration runner:

- creates the schema and `migrations` table if they are missing
- checks that PostgreSQL is version 12 or newer
- runs pending migrations in order inside transactions
- records each migration id and whether it is breaking
- rejects a database revision that includes a breaking migration newer than the
  current crate supports
- warns, but continues, when the database has a newer non-breaking revision than
  the current crate supports

Run migrations before starting workers after deploys that may include schema
changes:

```bash
graphile-worker migrate
```

Migration 11 has a specific safety check: if locked jobs are present and
PostgreSQL returns error code `22012`, migration fails with guidance to shut
workers down cleanly and unlock locked jobs and queues before retrying.

## Recovery and locked jobs

Worker recovery is opt-in. When enabled, workers register heartbeat rows in
PostgreSQL and refresh `last_heartbeat_at` on a configured interval. A sweeper
can recover jobs held by workers that stopped heartbeating, and jobs or queues
locked by worker ids that are no longer registered.

```rust,ignore
use graphile_worker::{WorkerOptions, WorkerRecoveryConfig};
use std::time::Duration;

let recovery = WorkerRecoveryConfig::default()
    .enabled(true)
    .heartbeat_interval(Duration::from_secs(30))
    .sweep_interval(Duration::from_secs(60))
    .sweep_threshold(Duration::from_secs(300))
    .recovery_delay(Duration::from_secs(30));

let worker = WorkerOptions::default()
    .worker_recovery(recovery)
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Recovered jobs are unlocked, their attempt count is decremented back, their
queue lock is released, and `run_at` is delayed by the configured recovery
delay. Sweeps use a transaction-scoped PostgreSQL advisory lock so only one
sweeper performs recovery at a time.

Manual recovery is available even when automatic recovery is disabled:

```rust,ignore
use graphile_worker::SweepStaleWorkersOptions;
use std::time::Duration;

let result = worker
    .create_utils()
    .sweep_stale_workers(SweepStaleWorkersOptions {
        sweep_threshold: Some(Duration::from_secs(300)),
        recovery_delay: Some(Duration::from_secs(30)),
        dry_run: true,
    })
    .await?;

println!("Would recover workers: {:?}", result.worker_ids);
```

Use `force-unlock` only when you have identified worker ids that should no
longer own locks:

```bash
graphile-worker workers
graphile-worker force-unlock graphile_worker_deadbeef
```

## Local queue considerations

The local queue reduces database round trips by batch-fetching jobs and caching
them in the worker process. It has operational implications:

- unclaimed jobs are returned to PostgreSQL on shutdown or when the local queue
  TTL expires
- refetch delay can reduce thundering-herd behavior when queues are empty
- cache size and refill threshold should be chosen with worker concurrency and
  database load in mind

```rust,ignore
use graphile_worker::{LocalQueueConfig, RefetchDelayConfig, WorkerOptions};
use std::time::Duration;

let worker = WorkerOptions::default()
    .local_queue(
        LocalQueueConfig::default()
            .with_size(100)
            .with_ttl(Duration::from_secs(300))
            .with_refetch_delay(
                RefetchDelayConfig::default()
                    .with_duration(Duration::from_millis(100))
                    .with_threshold(10),
            ),
    )
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

## Production readiness checklist

Before sending production traffic to workers, verify the following:

- PostgreSQL is version 12 or newer.
- The database URL is configured through `DATABASE_URL` or an equivalent
  application secret.
- Migrations are run before workers start processing jobs.
- All deployed workers agree on the Graphile Worker schema name.
- Worker task handlers are registered for every task identifier you enqueue.
- Concurrency, PostgreSQL pool size, and local queue size are sized together.
- Shutdown behavior is wired into your process manager or host application.
- Recovery is enabled when the deployment model can leave jobs locked after
  crashes, network partitions, forced aborts, or orchestrator shutdowns.
- Admin UI access uses Basic, Bearer, or header auth when exposed beyond
  loopback.
- Admin UI read-only mode is used for inspection-only deployments.
- Maintenance commands such as `cleanup`, `force-unlock`, and
  `sweep-stale-workers` are restricted to operators who can safely mutate queue
  state.
- Stale-worker sweeps are dry-run before recovery during incident response.
- Cleanup policy is understood before permanently failed jobs, task
  identifiers, or job queues are garbage-collected.
- Observability covers worker startup, job failures, locked workers, queue
  backlog, and migration failures.
