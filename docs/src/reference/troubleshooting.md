# Troubleshooting

This page collects common setup, runtime, and documentation issues with
concrete checks you can run before digging into application-specific code.

## Database Connection Problems

Graphile Worker RS needs a PostgreSQL connection. Most examples use a SQLx pool
and most local checks use `DATABASE_URL`.

Check that the URL is set and points at PostgreSQL:

```bash
echo "$DATABASE_URL"
```

For a local throwaway database that matches the test setup, the repository uses
PostgreSQL on port `54233`:

```bash
docker run -d --name graphile-worker-rs-test \
  -p 54233:5432 \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=postgres \
  postgres
```

Then use:

```bash
export DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres'
```

If a command cannot connect, verify that the container is ready:

```bash
docker exec graphile-worker-rs-test pg_isready -U postgres -h localhost
```

## Migrations Have Not Run

If jobs are not visible, worker startup fails, or CLI commands fail against an
empty database, run the Graphile Worker migrations for the target database.

Using the CLI:

```bash
graphile-worker --database-url postgres://postgres:postgres@localhost/postgres migrate
```

Or with `DATABASE_URL`:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost/postgres graphile-worker migrate
```

By default, the CLI uses the `graphile_worker` schema. See
[Migrations](../operations/migrations.md) and [CLI](../operations/cli.md) for
more operational detail.

## Runtime Or Feature Mismatches

The default crate features are Tokio, rustls TLS, and the SQLx driver:

```toml
graphile_worker = { version = "0.13" }
```

If you disable default features, choose one runtime and one supported driver.
For async-std with SQLx, include the async-std runtime feature and a TLS feature:

```toml
graphile_worker = { version = "0.13", default-features = false, features = ["runtime-async-std", "driver-sqlx", "tls-rustls"] }
async-std = { version = "1", features = ["attributes"] }
```

For Tokio with the `tokio-postgres` driver:

```toml
graphile_worker = { version = "0.13", default-features = false, features = ["runtime-tokio", "driver-tokio-postgres"] }
```

The repository test matrix only runs `driver-tokio-postgres` with
`runtime-tokio`. If you see feature errors, compare your feature list with
[Runtime and Drivers](../configuration/runtime-drivers.md) and
[Features](features.md).

## Worker Starts But A Job Does Not Run

First check that the task identifier used when adding the job exactly matches
the registered handler:

```rust,ignore
impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}
```

The worker must register that handler before `init()`:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Then add jobs with the same identifier or with the type-safe helper:

```rust,ignore
worker
    .create_utils()
    .add_raw_job(
        "send_email",
        serde_json::json!({ "to": "user@example.com" }),
        Default::default(),
    )
    .await?;
```

If jobs are scheduled for the future, they will not run until their `run_at`
time. If jobs share a queue name, jobs in that queue run in series rather than
in parallel. See [Tasks](../concepts/tasks.md), [Scheduling](../concepts/scheduling.md),
and [Queues](../concepts/queues.md).

## Payload Deserialization Failures

Task payloads are deserialized into the task struct. If the payload shape does
not match the struct, the handler cannot receive the data you expect.

Check the struct fields and the JSON payload side by side:

```rust,ignore
#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
    body: String,
}
```

```sql
SELECT graphile_worker.add_job(
    'send_email',
    json_build_object(
        'to', 'user@example.com',
        'subject', 'Welcome',
        'body', 'Thanks for signing up.'
    )
);
```

When adding jobs from Rust, prefer the type-safe `add_job` path when the task
type is available:

```rust,ignore
utils.add_job(
    SendEmail {
        to: "user@example.com".to_string(),
        subject: "Welcome".to_string(),
        body: "Thanks for signing up.".to_string(),
    },
    Default::default(),
).await?;
```

## Shutdown Signal Conflicts

By default, Graphile Worker installs OS-level shutdown listeners such as
`SIGINT` and `SIGTERM` so it can drain gracefully. If your application already
owns shutdown handling, disable the built-in listeners and pass your own
shutdown future:

```rust,ignore
let shutdown = WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(false)
    .shutdown_signal(on_shutdown());

let worker = WorkerOptions::default()
    .worker_shutdown(shutdown)
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

See [Shutdown](../configuration/shutdown.md) for the configuration surface.

## Locked Jobs After A Crash

Worker recovery is disabled by default. For deployments where jobs may remain
locked after a process crash, network partition, forced abort, or orchestrator
shutdown, enable recovery:

```rust,ignore
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

The CLI can also inspect and recover stale workers:

```bash
graphile-worker sweep-stale-workers --dry-run
graphile-worker sweep-stale-workers --sweep-threshold 5m --recovery-delay 30s
```

Use `--dry-run` first when you want to see which workers would be recovered
without unlocking or rescheduling jobs. See [Recovery](../guides/recovery.md).

## CLI Checks

The installed binary is `graphile-worker`. It connects with `--database-url` or
`DATABASE_URL`.

Useful checks:

```bash
graphile-worker list --state ready
graphile-worker show 123
graphile-worker stats
graphile-worker queues
graphile-worker workers
```

Useful repair commands:

```bash
graphile-worker fail 125 --reason "invalid payload"
graphile-worker reschedule 126 --run-at 2026-01-02T03:04:05Z
graphile-worker force-unlock graphile_worker_deadbeef
graphile-worker cleanup
```

Use the same database URL and schema that your worker uses, otherwise the CLI
will inspect a different queue.

## Local Test Environment Checks

The repository has two common test paths:

```bash
just test
```

This runs `cargo test --all` and requires `DATABASE_URL` to point at a usable
PostgreSQL database.

```bash
just test-docker
```

This starts a `postgres` container named `graphile-worker-rs-test`, waits for
`pg_isready`, runs tests with
`DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres'`, and then
removes the container.

If Docker reports that the container name or port is already in use, remove the
old test container or stop the process using port `54233`:

```bash
docker rm -f graphile-worker-rs-test
```

Runtime-specific test helpers also exist:

```bash
just test-docker-runtime runtime-tokio driver-sqlx
just test-docker-runtime runtime-async-std driver-sqlx
just test-docker-runtime runtime-tokio driver-tokio-postgres
```

## Documentation Build Checks

Documentation commands are defined in the repository justfile:

```bash
just docs-install
just docs
just docs-serve
```

`just docs-install` installs `mdbook` with version `^0.4`. `just docs` runs
`mdbook build docs`. `just docs-serve` serves the book on `127.0.0.1:3000` by
default.
