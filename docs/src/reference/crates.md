# Crate Map

Graphile Worker RS is published as a main application crate plus smaller crates
that hold reusable pieces of the worker, cron, migration, admin, and database
layers. Most applications should depend on `graphile_worker`; the other crates
are useful when you need a narrower API surface, are building tooling around the
worker schema, or are working on Graphile Worker RS itself.

## Main User-Facing Crate

| Crate | Use it for |
| --- | --- |
| `graphile_worker` | Running workers, adding jobs, defining tasks, configuring cron, applying migrations, and using the public worker API from one dependency. |

The root crate re-exports the core types that applications normally need:

- `Worker` and `WorkerOptions` for worker setup.
- `WorkerUtils` for job management.
- `TaskHandler` and related task handler types.
- `JobSpec` and job data types.
- `Cron`, crontab parser and crontab value types.
- Lifecycle hook, context, database, and shutdown signal types.
- Dead worker recovery types such as `SweepStaleWorkersOptions`.

For a normal worker binary, start here:

```rust,ignore
use graphile_worker::WorkerOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let worker = WorkerOptions::default()
        .database_url("postgres://postgres:postgres@localhost/postgres")
        .define_job::<SendEmail>()
        .init()
        .await?;

    worker.run().await?;
    Ok(())
}
```

For job management without running a worker loop, use `WorkerUtils` from the
same crate:

```rust,ignore
use graphile_worker::{JobSpec, WorkerUtils};

async fn enqueue(utils: &WorkerUtils) -> graphile_worker::errors::Result<()> {
    utils
        .add_raw_job(
            "send_email",
            serde_json::json!({ "to": "user@example.com" }),
            JobSpec::builder().queue_name("mailers").build(),
        )
        .await?;

    Ok(())
}
```

## User-Facing Building Blocks

These crates provide the types application code is most likely to see through
`graphile_worker` re-exports. Depend on them directly only when you are sharing
small interfaces across crates and do not want the full worker dependency.

| Crate | What it contains |
| --- | --- |
| `graphile_worker_task_handler` | `TaskHandler` and reusable `JobDefinition` values for defining job handlers. |
| `graphile_worker_job_spec` | Job options such as queue name, run time, max attempts, priority, and deduplication keys. |
| `graphile_worker_job` | Job row/data types used by handlers, hooks, queries, and utilities. |
| `graphile_worker_ctx` | Worker context passed through job execution. |
| `graphile_worker_extensions` | Extension storage used to attach and retrieve shared application state. |
| `graphile_worker_lifecycle_hooks` | Hook registry and lifecycle events around worker startup, job execution, completion, and cron ticks. |
| `graphile_worker_shutdown_signal` | Cross-platform shutdown signal handling for graceful worker shutdown. |

For example, a helper crate that only defines task handlers can depend on the
task handler and context crates instead of depending on the whole worker:

```rust,ignore
use graphile_worker_ctx::WorkerContext;
use graphile_worker_task_handler::{IntoTaskHandlerResult, TaskHandler};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let _ = (self.to, ctx);
        Ok::<(), String>(())
    }
}
```

## Cron Crates

Cron support is split into parser, shared types, and runner pieces.

| Crate | What it contains |
| --- | --- |
| `graphile_worker_crontab_types` | Crontab value types and utilities, including timers, fields, fills, and job key mode types. |
| `graphile_worker_crontab_parser` | Parser for crontab configuration. |
| `graphile_worker_crontab_runner` | Runner that schedules crontab entries against the worker database and lifecycle hooks. |

Applications usually get these through `graphile_worker`, which re-exports
`parse_crontab`, `Cron`, `CronBuilder`, `Crontab`, `CrontabTimer`, and related
types.

## Support Crates

These crates are useful for tools, integrations, and advanced applications that
need to manage jobs or schema state without pulling every top-level worker API
into their own public surface.

| Crate | What it contains |
| --- | --- |
| `graphile_worker_utils` | Job management helpers used by `WorkerUtils`, including adding, removing, rescheduling, and other worker utility operations. |
| `graphile_worker_queries` | Database query helpers for Graphile Worker job and worker operations. |
| `graphile_worker_recovery` | Dead worker recovery helpers, including stale worker sweeping and recovery configuration. |
| `graphile_worker_migrations` | Database migrations for the Graphile Worker schema. |

The root crate's documentation describes the managed schema as using the
`graphile_worker` schema by default, with private jobs, tasks, job queues, and
workers tables. These support crates are the lower-level pieces used to manage
that schema and operate on its rows.

## Internal-ish Infrastructure Crates

These crates are part of the workspace API surface, but most applications should
not need to depend on them directly.

| Crate | What it contains |
| --- | --- |
| `graphile_worker_database` | Database driver abstraction used by worker, migrations, queries, recovery, cron, and utilities. It has feature support for `sqlx` and `tokio-postgres` drivers. |
| `graphile_worker_runtime` | Async runtime compatibility helpers for Tokio and async-std feature combinations. |
| `graphile_worker_task_details` | Shared mapping between task IDs and task identifiers. |
| `graphile_worker_migrations_core` | Core migration types used by the migrations crate. |
| `graphile_worker_migrations_macros` | Procedural macros used by the migrations crate. |

These crates exist to keep feature flags and dependencies focused. For example,
database-facing crates share the `driver-sqlx`, `driver-tokio-postgres`,
`runtime-tokio`, `runtime-async-std`, `tls-rustls`, and `tls-native-tls` feature
families instead of hard-coding one driver and runtime everywhere.

## Admin And CLI Crates

The workspace also includes crates for operational tools around a Graphile
Worker installation.

| Crate | What it contains |
| --- | --- |
| `graphile_worker_cli` | The `graphile-worker` binary for migrations, job management, admin server startup, stats, queues, workers, cleanup, and stale worker sweeping. |
| `graphile_worker_admin_api` | Shared admin API contracts and queries. |
| `graphile_worker_admin_ui` | Embedded Leptos admin UI and server-side assets. |
| `graphile_worker_admin_ui_client` | Leptos WASM client for the embedded admin UI. |

The CLI binary is installed as `graphile-worker` and connects with
`--database-url` or `DATABASE_URL`. Its README lists commands such as:

```bash
graphile-worker migrate
graphile-worker add send_email --payload '{"to":"user@example.com"}'
graphile-worker list --state ready
graphile-worker complete 123 124
graphile-worker fail 125 --reason "invalid payload"
graphile-worker reschedule 126 --run-at 2026-01-02T03:04:05Z
graphile-worker sweep-stale-workers --dry-run
```

## Choosing A Dependency

Use `graphile_worker` unless you have a concrete reason not to. It is the stable
entry point for application code and re-exports the main worker, task, job,
cron, migration, hook, database, recovery, and utility types.

Use a smaller crate directly when:

- A library crate only needs to expose task handler types.
- A tool only needs job specs, job row types, or query helpers.
- An integration needs admin API contracts without embedding the UI.
- You are extending Graphile Worker RS itself and need the same internal
  boundaries as the workspace.
