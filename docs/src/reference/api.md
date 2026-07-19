# API Reference

This page is a map of the public Rust API. For complete signatures, trait
bounds, and generated item documentation, use the docs.rs links in each section.

Most applications should start with the root crate:

- [`graphile_worker`](https://docs.rs/graphile_worker/) - worker setup, task registration, job scheduling, cron helpers, lifecycle hooks, shutdown, recovery, and the most commonly used re-exports.

## Worker Setup

Use [`WorkerOptions`](https://docs.rs/graphile_worker/latest/graphile_worker/struct.WorkerOptions.html) to configure and initialize a worker, then call [`Worker::run`](https://docs.rs/graphile_worker/latest/graphile_worker/struct.Worker.html) or `Worker::run_once`.

Important public types:

- `WorkerOptions` - builder-style worker configuration.
- `WorkerBuildError` - initialization errors.
- `Worker` - initialized worker runtime.
- `WorkerShutdownConfig` - graceful shutdown behavior.
- `ShutdownSignal` - shareable shutdown future.

Common configuration methods include:

- `database_url`, `database`, and `pg_pool` for database connections.
- `schema`, `concurrency`, `poll_interval`, `max_pg_conn`, and `use_local_time`.
- `define_job`, `define_batch_job`, and `define_jobs` for task registration.
- `add_forbidden_flag` for workers that skip jobs with specific flags.
- `local_queue`, `complete_job_batch_delay`, and `fail_job_batch_delay` for throughput tuning.
- `worker_recovery`, `heartbeat_interval`, `sweep_interval`, `sweep_threshold`, and `recovery_delay` for dead worker recovery.
- `listen_os_shutdown_signals`, `shutdown_signal`, `shutdown_grace_period`, and `shutdown_interrupted_job_retry_delay` for shutdown control.

```rust,ignore
use graphile_worker::{
    IntoTaskHandlerResult, TaskHandler, WorkerContext, WorkerOptions,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}

let worker = WorkerOptions::default()
    .database_url("postgres://postgres:postgres@localhost/postgres")
    .schema("graphile_worker")
    .concurrency(8)
    .poll_interval(Duration::from_millis(500))
    .define_job::<SendEmail>()
    .init()
    .await?;

worker.run().await?;
```

## Task Handlers

Task handler APIs live in [`graphile_worker_task_handler`](https://docs.rs/graphile_worker_task_handler/) and are re-exported by `graphile_worker`.

Important public types:

- `TaskHandler` - trait for a single job payload type.
- `IntoTaskHandlerResult` - conversion trait for task return values such as `()` and `Result<(), E>`.
- `BatchTaskHandler` - trait for JSON-array batch payloads.
- `BatchTaskResult` and `IntoBatchTaskHandlerResult` - batch completion and partial failure results.
- `JobDefinition` - reusable task registration value.
- `TaskHandlerOutcome` and `TaskHandlerFn` - type-erased handler output and function type.
- `run_task_from_worker_ctx` - helper that deserializes and runs a `TaskHandler` from `WorkerContext`.

Batch handlers operate on `Vec<Self>` item payloads. `BatchTaskResult::ItemResults`
must return one result for each input item, in the same order; failed items are
retried as a replacement JSON-array payload.

```rust,ignore
use graphile_worker::{
    BatchTaskHandler, IntoBatchTaskHandlerResult, WorkerContext, WorkerOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
struct IndexRecord {
    id: i64,
}

impl BatchTaskHandler for IndexRecord {
    const IDENTIFIER: &'static str = "index_record";

    async fn run_batch(
        items: Vec<Self>,
        _ctx: WorkerContext,
    ) -> impl IntoBatchTaskHandlerResult {
        let results = items
            .into_iter()
            .map(|item| {
                if item.id > 0 {
                    Ok(())
                } else {
                    Err("invalid id")
                }
            })
            .collect::<Vec<_>>();

        results
    }
}

let options = WorkerOptions::default().define_batch_job::<IndexRecord>();
```

## Job Scheduling And Management

Use [`WorkerUtils`](https://docs.rs/graphile_worker/latest/graphile_worker/struct.WorkerUtils.html) to add, update, and maintain jobs without running a worker loop in the same code path.

Important public types:

- `WorkerUtils` - client for scheduling and maintenance operations.
- `WorkerUtilsWithExecutor` - scoped client that runs supported utility operations
  through a caller-provided executor or transaction.
- `JobSpec` and `JobSpecBuilder` - optional job parameters.
- `JobKeyMode` - job-key behavior: `Replace`, `PreserveRunAt`, or `UnsafeDedupe`.
- `RawJobSpec` - raw batch scheduling input.
- `Job`, `JobBuilder`, `DbJob`, and `DbJobData` - job records returned by scheduling and management APIs.
- `CleanupTask` - cleanup operations from `graphile_worker_utils::types`.
- `RescheduleJobOptions` - optional fields for rescheduling jobs.

Common `WorkerUtils` methods include:

- `add_job`, `add_raw_job`, `add_jobs`, `add_raw_jobs`, and `add_batch_job`.
- `remove_job`, `complete_jobs`, `permanently_fail_jobs`, and `reschedule_jobs`.
- `list_active_workers`, `sweep_stale_workers`, `sweep_stale_workers_with_config`, and `force_unlock_workers`.
- `cleanup` and `migrate`.

Call `WorkerUtils::with_executor` to run scheduling and direct job-management
methods through an existing `DbExecutorArg`. The returned facade does not expose
`cleanup`, `migrate`, or stale-worker sweeps because those operations manage their
own transaction or shared maintenance state.

```rust,ignore
use graphile_worker::{JobKeyMode, JobSpec, WorkerUtils};
use serde_json::json;

let spec = JobSpec::builder()
    .queue_name("email")
    .job_key("welcome-email:user-123")
    .job_key_mode(JobKeyMode::PreserveRunAt)
    .max_attempts(5)
    .priority(-10)
    .build();

let job = utils
    .add_raw_job(
        "send_email",
        json!({ "to": "user@example.com" }),
        spec,
    )
    .await?;
```

## Worker Context And Extensions

Context APIs live in [`graphile_worker_ctx`](https://docs.rs/graphile_worker_ctx/) and are re-exported by `graphile_worker`.

Important public types:

- `WorkerContext` - runtime context passed to task handlers.
- `WorkerContextBuilder` - builder for creating contexts.
- `WorkerContextExt` - root-crate extension helpers for worker context.
- `TaskDetails` and `SharedTaskDetails` - task identifier mappings.
- `Extensions` and `ReadOnlyExtensions` from [`graphile_worker_extensions`](https://docs.rs/graphile_worker_extensions/) - typed application state storage.

Add shared state with `WorkerOptions::add_extension`. Task handlers receive a
`WorkerContext` and can use the context APIs exposed by the generated docs.

## Cron

Cron support is exposed from the root crate and split across three crates:

- [`graphile_worker_crontab_parser`](https://docs.rs/graphile_worker_crontab_parser/) - `parse_crontab` and `CrontabParseError`.
- [`graphile_worker_crontab_types`](https://docs.rs/graphile_worker_crontab_types/) - `Crontab`, `CrontabField`, `CrontabFill`, `CrontabTimer`, `CrontabTimerError`, `CrontabValue`, and cron `JobKeyMode`.
- [`graphile_worker_crontab_runner`](https://docs.rs/graphile_worker_crontab_runner/) - `CronRunner`, `cron_main`, `Clock`, `MockClock`, `KnownCrontab`, and `ScheduleCronJobError`.

The root crate also re-exports:

- `Cron` and `CronBuilder` for typed schedules.
- `CronInput` for values accepted by `WorkerOptions::with_cron`.
- `CronJobKeyMode`, which is the crontab `JobKeyMode` re-exported under a distinct root-crate name.

```rust,ignore
use graphile_worker::{Cron, CrontabFill, WorkerOptions};

let options = WorkerOptions::default()
    .define_job::<SendEmail>()
    .with_cron(
        Cron::daily_at::<SendEmail>(8, 0)?
            .fill(CrontabFill::hours(1)),
    );

let options = WorkerOptions::default()
    .with_cron("0 8 * * * send_email")?;
```

## Database And Migrations

The database abstraction lives in [`graphile_worker_database`](https://docs.rs/graphile_worker_database/).

Important public types:

- `Database`, `DatabaseDriver`, `DbTransaction`, and `TransactionDriver`.
- `DbExecutor` and `DbExecutorArg`.
- `DbError`.
- `Schema`.
- `Notification` and `NotificationStream`.
- `DbCell`, `DbParams`, `DbRow`, `DbValue`, and `FromDbCell`.
- `escape_identifier`.

Driver modules are feature gated:

- `graphile_worker_database::sqlx` with the `driver-sqlx` feature.
- `graphile_worker_database::tokio_postgres` with the `driver-tokio-postgres` feature.

Migrations live in [`graphile_worker_migrations`](https://docs.rs/graphile_worker_migrations/).

Important public items:

- `migrate` - runs Graphile Worker schema migrations.
- `MigrateError` - migration error type.
- `GraphileWorkerMigration` - migration metadata type.
- `pg_version` and `sql` modules.

The migration support crates are also public:

- [`graphile_worker_migrations_core`](https://docs.rs/graphile_worker_migrations_core/) - `GraphileWorkerMigration`.
- [`graphile_worker_migrations_macros`](https://docs.rs/graphile_worker_migrations_macros/) - `include_migrations!`.

## Lifecycle Hooks

Lifecycle hook APIs live in [`graphile_worker_lifecycle_hooks`](https://docs.rs/graphile_worker_lifecycle_hooks/) and are re-exported by `graphile_worker`.

Important public types:

- `HookRegistry` - registry for hook handlers.
- `Plugin` - plugin trait for registering hooks.
- `Event`, `HookOutput`, and `Interceptable` - hook event traits.
- `TypeErasedHooks` - erased hook registry.
- `HookResult` and event-specific result types such as `JobScheduleResult`.
- Event marker and context types exported from the crate's `events` and `context` modules.

Workers can register individual handlers with `WorkerOptions::on` or register a
plugin with `WorkerOptions::add_plugin`.

```rust,ignore
use graphile_worker::{HookResult, JobStart, WorkerOptions};

let options = WorkerOptions::default()
    .on(JobStart, |ctx| async move {
        println!("starting job {}", ctx.job.id());
    })
    .on(graphile_worker::BeforeJobRun, |ctx| async move {
        if ctx.payload.get("skip").and_then(|value| value.as_bool()) == Some(true) {
            return HookResult::Skip;
        }

        HookResult::Continue
    });
```

## Recovery And Shutdown

Recovery APIs are exposed from [`graphile_worker_recovery`](https://docs.rs/graphile_worker_recovery/) and re-exported by the root crate where they are part of the worker API.

Important public types:

- `WorkerRecoveryConfig` - dead worker recovery settings.
- `SweepStaleWorkersOptions` and `SweepStaleWorkersResult` - manual sweep input and output.
- `ResolvedSweepConfig` - resolved sweep settings.
- `ActiveWorkerRow` - heartbeat row returned by worker listing APIs.
- `INFRASTRUCTURE_RESILIENT_FLAG` and `job_has_resilient_flag` - infrastructure-resilient job flag support.

Shutdown signal support lives in [`graphile_worker_shutdown_signal`](https://docs.rs/graphile_worker_shutdown_signal/):

- `ShutdownSignal` - cloneable shared future.
- `shutdown_signal` - OS shutdown signal detector.

## Runtime

[`graphile_worker_runtime`](https://docs.rs/graphile_worker_runtime/) provides
runtime-neutral async building blocks used by the worker internals and exposed as
a public crate.

Important public items:

- `channel`, `Receiver`, and `Sender`.
- `Mutex`, `RwLock`, `RwLockReadGuard`, and `RwLockWriteGuard`.
- `Notify` and `Notified`.
- `spawn`, `AbortHandle`, `JoinHandle`, and `JoinError`.
- `interval`, `sleep`, `sleep_until`, `timeout_at`, `Interval`, and `TimeoutError`.

The crate requires either the `runtime-tokio` or `runtime-async-std` feature.

## Admin Crates

Admin APIs are published as separate crates:

- [`graphile_worker_admin_api`](https://docs.rs/graphile_worker_admin_api/) - shared admin request and response DTO modules, plus SQLx read-query helpers behind its `sqlx` feature.
- [`graphile_worker_admin_ui`](https://docs.rs/graphile_worker_admin_ui/) - native Axum admin server crate.
- [`graphile_worker_admin_ui_client`](https://docs.rs/graphile_worker_admin_ui_client/) - admin UI client crate, including `manifest_dir` and WASM-only client code.

## Feature Flags

The root crate default feature set is:

```toml
default = ["runtime-tokio", "tls-rustls", "driver-sqlx"]
```

Runtime features:

- `runtime-tokio`
- `runtime-async-std`

TLS features:

- `tls-rustls`
- `tls-native-tls`

Database driver features:

- `driver-sqlx`
- `driver-tokio-postgres`

OpenTelemetry compatibility features:

- `opentelemetry_0_30`
- `opentelemetry_0_31`
- `opentelemetry_0_32`

## Lower-Level Crates

These crates are public for advanced integration and internal composition, but
most applications use them through `graphile_worker` re-exports:

- [`graphile_worker_job`](https://docs.rs/graphile_worker_job/) - `Job`, `JobBuilder`, `DbJob`, and `DbJobData`.
- [`graphile_worker_job_spec`](https://docs.rs/graphile_worker_job_spec/) - `JobSpec`, `JobSpecBuilder`, and `JobKeyMode`.
- [`graphile_worker_task_details`](https://docs.rs/graphile_worker_task_details/) - `TaskDetails` and `SharedTaskDetails`.
- [`graphile_worker_queries`](https://docs.rs/graphile_worker_queries/) - lower-level query modules such as `add_job`, `get_job`, `complete_job`, `fail_job`, `return_jobs`, `recover_workers`, `task_identifiers`, and `worker_heartbeat`.
- [`graphile_worker_utils`](https://docs.rs/graphile_worker_utils/) - `WorkerUtils`, `client`, and `types`.
