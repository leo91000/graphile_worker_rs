# Application State and Extensions

Task handlers often need access to dependencies that are not part of the job
payload: configuration, API clients, counters, caches, or other process-local
state. Graphile Worker RS exposes this through worker extensions.

An extension is a value registered on `WorkerOptions` and later read from
`WorkerContext` by type. Each handler receives a `WorkerContext`, so the same
shared state is available wherever jobs run in that worker process.

## Register Shared State

Register application state with `WorkerOptions::add_extension` before calling
`init()`.

```rust,ignore
use graphile_worker::WorkerOptions;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

#[derive(Clone, Debug)]
struct AppState {
    run_count: Arc<AtomicUsize>,
}

impl AppState {
    fn new() -> Self {
        Self {
            run_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn increment_run_count(&self) -> usize {
        self.run_count.fetch_add(1, SeqCst)
    }
}

let worker = WorkerOptions::default()
    .define_job::<ShowRunCount>()
    .pg_pool(pg_pool)
    .add_extension(AppState::new())
    .init()
    .await?;
```

Extension values are stored by Rust type. `add_extension` accepts values that
are `Clone + Send + Sync + Debug + 'static`. If another value with the same type
is inserted into the same extension set, the later value replaces the earlier
one.

Use a wrapper type when you need to store two values with the same underlying
type:

```rust,ignore
#[derive(Clone, Debug)]
struct PublicApiBaseUrl(String);

#[derive(Clone, Debug)]
struct InternalApiBaseUrl(String);
```

## Read State From a Handler

Inside a task handler, call `ctx.get_ext::<T>()` to retrieve an extension by
type. The method returns `Option<&T>`, so handlers can decide whether missing
state is a job error or an application configuration error.

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, WorkerContext};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct ShowRunCount;

impl TaskHandler for ShowRunCount {
    const IDENTIFIER: &'static str = "show_run_count";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let app_state = ctx
            .get_ext::<AppState>()
            .ok_or_else(|| "AppState extension is not configured".to_string())?;

        let run_count = app_state.increment_run_count();
        println!("Run count: {run_count}");

        Ok::<(), String>(())
    }
}
```

Handlers receive read-only access to the extension container. If the state
itself must be mutated or shared across concurrent jobs, put the synchronization
inside your state type, for example with `Arc`, atomics, or other thread-safe
interior mutability.

## What WorkerContext Provides

`WorkerContext` is the per-job context passed to every task handler. In addition
to extensions, it exposes job and worker data such as:

- `ctx.payload()` for the JSON payload.
- `ctx.job()` for the complete job record.
- `ctx.database()` for the database handle.
- `ctx.pg_pool()` when the SQLx driver feature is used.
- `ctx.schema()` for the configured Graphile Worker schema.
- `ctx.worker_id()` for the worker processing the job.
- `ctx.get_ext::<T>()` for application-specific extensions.

Use extensions for dependencies owned by your application. Use the built-in
context methods for worker metadata and database access.

## Context Helper Pattern

Some handlers need to enqueue follow-up work. Import `WorkerContextExt` to use
the helper methods implemented for `WorkerContext`.

```rust,ignore
use chrono::{offset::Utc, Duration};
use graphile_worker::{
    IntoTaskHandlerResult, JobSpecBuilder, WorkerContext, WorkerContextExt,
};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
struct SendWs {
    request_id: String,
}

impl TaskHandler for SendWs {
    const IDENTIFIER: &'static str = "send_ws";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("[send_ws] sent request {}", self.request_id);

        ctx.add_job(
            CheckWs {
                request_id: self.request_id,
            },
            JobSpecBuilder::new()
                .run_at(Utc::now() + Duration::seconds(10))
                .build(),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }
}

#[derive(Deserialize, Serialize, Clone)]
struct CheckWs {
    request_id: String,
}
```

The helper trait creates a `WorkerUtils` value from the current context, using
the same database, schema, task details, and local-time setting as the running
worker. It provides typed and raw job enqueueing helpers:

- `ctx.utils()`
- `ctx.add_job(...)`
- `ctx.add_raw_job(...)`
- `ctx.add_jobs(...)`
- `ctx.add_raw_jobs(...)`
- `ctx.add_batch_job(...)`

This pattern keeps handler code small: use extensions for application-owned
dependencies, and use `WorkerContextExt` when the handler needs to schedule more
work through the same worker configuration.
