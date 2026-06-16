# Observability

Graphile Worker RS exposes observability through standard Rust `tracing`, optional
OpenTelemetry compatibility features, and lifecycle hooks. The worker does not
force a logging or metrics backend on applications; install the subscriber,
exporter, and hook plugin that match your runtime.

## Tracing

The main crate depends on `tracing`, so worker internals and application task
handlers can emit spans and events into the subscriber you install. The hooks
example uses `tracing-subscriber` with an environment filter and a formatted
output layer:

```rust,ignore
use tracing_subscriber::{
    filter::EnvFilter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

pub fn enable_logs() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
```

Initialize the subscriber once, before starting the worker. The example keeps
SQLx noise at `warn` while enabling debug logging elsewhere.

## OpenTelemetry compatibility

OpenTelemetry support is feature-gated. Enable exactly one compatibility
feature:

```toml
[dependencies]
graphile_worker = {
    version = "0.13",
    features = ["opentelemetry_0_32"]
}
```

Available compatibility features are:

| Feature | OpenTelemetry crate version |
| --- | --- |
| `opentelemetry_0_30` | `opentelemetry` 0.30 |
| `opentelemetry_0_31` | `opentelemetry` 0.31 |
| `opentelemetry_0_32` | `opentelemetry` 0.32 |

The crate rejects builds that enable more than one of these features at the
same time.

When an OpenTelemetry feature is enabled, job insertion code can capture the
current span context and write it into the job payload under `_trace`. Object
payloads receive the field directly. Array payloads receive it on each object
item in the array, while scalar values are left unchanged.

```json
{
  "user_id": 42,
  "_trace": {
    "flags": 1,
    "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
    "span_id": "00f067aa0ba902b7"
  }
}
```

When the worker later runs a job, it reads `_trace` from object payloads and
adds the recorded span context as a link to the current span. Invalid,
non-object, or missing trace payloads are ignored. Without an OpenTelemetry
feature, this trace extraction and linking path is a no-op.

## Metrics with lifecycle hooks

Lifecycle hooks are the extension point for metrics and structured operational
events. A plugin registers callbacks on `HookRegistry`; those callbacks receive
context objects for worker and job events.

The metrics example counts started, completed, and failed jobs with atomics:

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use graphile_worker::{
    HookRegistry, JobComplete, JobFail, JobStart, Plugin, WorkerShutdown, WorkerStart,
};

pub struct MetricsPlugin {
    jobs_started: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
}

impl Plugin for MetricsPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            println!("[MetricsPlugin] Worker {} started", ctx.worker_id);
        });

        let jobs_started = Arc::new(self.jobs_started);
        let jobs_completed = Arc::new(self.jobs_completed);
        let jobs_failed = Arc::new(self.jobs_failed);

        {
            let jobs_started = jobs_started.clone();
            hooks.on(JobStart, move |ctx| {
                let jobs_started = jobs_started.clone();
                async move {
                    jobs_started.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} started (task: {})",
                        ctx.job.id(),
                        ctx.job.task_identifier()
                    );
                }
            });
        }

        {
            let jobs_completed = jobs_completed.clone();
            hooks.on(JobComplete, move |ctx| {
                let jobs_completed = jobs_completed.clone();
                async move {
                    jobs_completed.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} completed in {:?}",
                        ctx.job.id(),
                        ctx.duration
                    );
                }
            });
        }

        {
            let jobs_failed = jobs_failed.clone();
            hooks.on(JobFail, move |ctx| {
                let jobs_failed = jobs_failed.clone();
                async move {
                    jobs_failed.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} failed: {} (will_retry: {})",
                        ctx.job.id(),
                        ctx.error,
                        ctx.will_retry
                    );
                }
            });
        }

        hooks.on(WorkerShutdown, move |ctx| async move {
            println!(
                "[MetricsPlugin] Worker {} shutting down (reason: {:?})",
                ctx.worker_id, ctx.reason
            );
        });
    }
}
```

Use the same hook points to export counters, histograms, or structured events to
your metrics backend:

| Hook | Useful signal |
| --- | --- |
| `WorkerStart` | worker process started and has a worker id |
| `WorkerShutdown` | worker process is stopping and exposes the shutdown reason |
| `JobStart` | job id and task identifier started executing |
| `JobComplete` | job completed and exposes execution duration |
| `JobFail` | job failed, exposes the error and whether it will retry |

## Logging

For application logs, combine a subscriber with hook callbacks and task-handler
instrumentation. The hook contexts expose job ids, task identifiers, errors,
durations, worker ids, and shutdown reasons, which are the stable values to put
into operational logs.

For example, a `JobFail` hook can log the job id, error, and retry decision;
`JobComplete` can log duration; `WorkerShutdown` can log the shutdown reason.
The example code prints these values with `println!`, but production code will
usually emit `tracing` events or send them to a metrics/logging client.

## Admin and monitor visibility

The workspace includes `graphile_worker_admin_api`,
`graphile_worker_admin_ui`, and `graphile_worker_admin_ui_client` crates. Use
those admin surfaces for queue visibility, and use hooks and tracing for
process-local signals that admin views cannot infer on their own, such as
per-process shutdown reasons, local counters, and span links.

For runtime monitoring, the most useful signals visible from the current public
hooks are:

| Signal | Source |
| --- | --- |
| Worker started | `WorkerStart` hook |
| Worker shutdown reason | `WorkerShutdown` hook |
| Job throughput | `JobStart` and `JobComplete` hooks |
| Job failure rate | `JobFail` hook |
| Job duration | `JobComplete` hook |
| Trace continuity from enqueue to execution | `_trace` payload plus OpenTelemetry span links |
