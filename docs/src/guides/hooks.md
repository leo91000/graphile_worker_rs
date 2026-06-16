# Lifecycle Hooks

Lifecycle hooks let you observe worker activity or intercept specific points in
the job lifecycle. They are registered through plugins and run inside the
worker process that owns the hook registry.

Use hooks for logging, metrics, validation, schedule-time payload adjustment,
and local queue diagnostics. Keep hook handlers fast and focused: they run on
the worker path they observe or intercept.

## Registering Hooks

A plugin implements `Plugin` and adds handlers to the `HookRegistry`:

```rust,ignore
use graphile_worker::{HookRegistry, JobComplete, Plugin, WorkerStart};

struct LoggingPlugin;

impl Plugin for LoggingPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            println!("worker {} started", ctx.worker_id);
        });

        hooks.on(JobComplete, async |ctx| {
            println!("job {} completed in {:?}", ctx.job.id(), ctx.duration);
        });
    }
}
```

Add plugins when building the worker:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .define_job::<ProcessData>()
    .pg_pool(pg_pool)
    .add_plugin(LoggingPlugin)
    .init()
    .await?;
```

Multiple plugins can be added with repeated `.add_plugin(...)` calls.

## Observer vs Interceptor Hooks

Hooks are split into two categories.

Observer hooks return `()` and are for side effects. They are useful for
metrics, tracing, logging, and state snapshots. When an observer event is
emitted, every registered handler for that event is called.

Interceptor hooks return a result that can affect worker behavior. The common
result type is `HookResult`:

```rust,ignore
use graphile_worker::HookResult;

HookResult::Continue
HookResult::Skip
HookResult::Fail("reason".to_string())
```

`Continue` allows the operation to proceed. `Skip` and `Fail(...)` are terminal
results and stop the interceptor chain.

`BeforeJobSchedule` uses `JobScheduleResult` instead. It can continue with a
payload, skip scheduling, or fail scheduling:

```rust,ignore
use graphile_worker::JobScheduleResult;

JobScheduleResult::Continue(payload)
JobScheduleResult::Skip
JobScheduleResult::Fail("reason".to_string())
```

Schedule interceptors are chained through the payload. If one plugin returns
`JobScheduleResult::Continue(new_payload)`, the next schedule interceptor
receives that transformed payload. A terminal result stops later schedule
interceptors.

## Worker Hooks

Worker hooks observe the worker process itself.

| Hook | Context highlights | Type |
| --- | --- | --- |
| `WorkerInit` | `database`, `schema`, `concurrency` | observer |
| `WorkerStart` | `database`, `worker_id`, `extensions` | observer |
| `WorkerShutdown` | `database`, `worker_id`, `reason` | observer |
| `WorkerRecovered` | `worker_id`, `dead_worker_ids`, `jobs_recovered` | observer |

Example:

```rust,ignore
use graphile_worker::{HookRegistry, Plugin, WorkerShutdown, WorkerStart};

struct WorkerLogPlugin;

impl Plugin for WorkerLogPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            println!("worker {} started", ctx.worker_id);
        });

        hooks.on(WorkerShutdown, async |ctx| {
            println!("worker {} stopped: {:?}", ctx.worker_id, ctx.reason);
        });
    }
}
```

## Job Run Hooks

Run hooks observe or intercept jobs as they are fetched and executed.

| Hook | Context highlights | Type |
| --- | --- | --- |
| `JobFetch` | `job`, `worker_id` | observer |
| `BeforeJobRun` | `job`, `worker_id`, `payload` | interceptor |
| `JobStart` | `job`, `worker_id` | observer |
| `AfterJobRun` | `job`, `worker_id`, `result`, `duration` | interceptor |
| `JobComplete` | `job`, `worker_id`, `duration` | observer |
| `JobFail` | `job`, `worker_id`, `error`, `will_retry` | observer |
| `JobPermanentlyFail` | `job`, `worker_id`, `error` | observer |
| `JobInterrupted` | `job`, `worker_id`, `reason` | observer |

`BeforeJobRun` can prevent a task handler from running:

```rust,ignore
use graphile_worker::{BeforeJobRun, HookRegistry, HookResult, Plugin};

struct ValidationPlugin;

impl Plugin for ValidationPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobRun, async |ctx| {
            let should_skip = ctx
                .payload
                .get("skip")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);

            if should_skip {
                return HookResult::Skip;
            }

            let should_fail = ctx
                .payload
                .get("force_fail")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);

            if should_fail {
                return HookResult::Fail("forced failure by validation hook".into());
            }

            HookResult::Continue
        });
    }
}
```

When a `BeforeJobRun` hook skips a job, the task handler is not called. When it
fails a job, the task handler is also not called and the normal failure path is
used.

`AfterJobRun` sees the task result and duration. It also returns `HookResult`,
so it can leave the result alone with `Continue`, mark the job skipped with
`Skip`, or fail it with `Fail(...)`.

## Schedule Hooks

`BeforeJobSchedule` intercepts jobs before they are persisted. It receives the
task `identifier`, JSON `payload`, and `JobSpec`.

Use it to reject, validate, or transform scheduled jobs:

```rust,ignore
use graphile_worker::{BeforeJobSchedule, HookRegistry, JobScheduleResult, Plugin};

struct SchedulePolicyPlugin;

impl Plugin for SchedulePolicyPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobSchedule, async |ctx| {
            if ctx.identifier == "blocked_task" {
                return JobScheduleResult::Skip;
            }

            let mut payload = ctx.payload.clone();
            if let Some(object) = payload.as_object_mut() {
                object.insert("checked_by_hook".into(), serde_json::json!(true));
            }

            JobScheduleResult::Continue(payload)
        });
    }
}
```

If a schedule hook returns `Skip`, no job is inserted. If it returns
`Fail(...)`, scheduling returns an error. If it returns `Continue(payload)`,
that payload is used for the job and passed to any later schedule interceptor.

Cron scheduling also has observer hooks:

| Hook | Context highlights | Type |
| --- | --- | --- |
| `CronTick` | `timestamp`, `crontabs` | observer |
| `CronJobScheduled` | `crontab`, `scheduled_at` | observer |
| `BeforeJobSchedule` | `identifier`, `payload`, `spec` | interceptor |

## Local Queue Hooks

Local queue hooks observe the in-process queue used by a worker.

| Hook | Context highlights | Type |
| --- | --- | --- |
| `LocalQueueInit` | `worker_id` | observer |
| `LocalQueueSetMode` | `worker_id`, `old_mode`, `new_mode` | observer |
| `LocalQueueGetJobsComplete` | `worker_id`, `jobs_count` | observer |
| `LocalQueueReturnJobs` | `worker_id`, `jobs_count` | observer |
| `LocalQueueRefetchDelayStart` | `worker_id`, `duration`, `threshold`, `abort_threshold` | observer |
| `LocalQueueRefetchDelayAbort` | `worker_id`, `count`, `abort_threshold` | observer |
| `LocalQueueRefetchDelayExpired` | `worker_id` | observer |

The local queue mode values are `Starting`, `Polling`, `Waiting`,
`TtlExpired`, and `Released`.

Example:

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use graphile_worker::{
    HookRegistry, LocalQueueGetJobsComplete, LocalQueueSetMode, Plugin,
};

#[derive(Clone)]
struct QueueMetricsPlugin {
    max_fetched: Arc<AtomicUsize>,
}

impl Plugin for QueueMetricsPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let max_fetched = self.max_fetched.clone();
        hooks.on(LocalQueueGetJobsComplete, move |ctx| {
            let max_fetched = max_fetched.clone();
            async move {
                max_fetched.fetch_max(ctx.jobs_count, Ordering::Relaxed);
            }
        });

        hooks.on(LocalQueueSetMode, async |ctx| {
            println!(
                "local queue mode changed: {:?} -> {:?}",
                ctx.old_mode,
                ctx.new_mode
            );
        });
    }
}
```

## Recovery Hook

`JobRecovery` is an interceptor for recovered jobs. Its context includes the
job, the current `worker_id`, the `previous_worker_id`, and the recovery
`reason`.

It returns `JobRecoveryResult`:

```rust,ignore
use graphile_worker::JobRecoveryResult;

JobRecoveryResult::Default
JobRecoveryResult::Reschedule {
    run_at,
    attempts: Some(3),
}
JobRecoveryResult::FailWithBackoff
JobRecoveryResult::Skip
```

Use `Default` to keep the worker's standard recovery behavior. Use the other
variants only when the recovered job needs explicit policy.

## Practical Example

This plugin combines observer hooks for metrics with an interceptor for
payload-based validation:

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use graphile_worker::{
    BeforeJobRun, HookRegistry, HookResult, JobComplete, JobFail, JobStart, Plugin,
};

#[derive(Default)]
struct Counters {
    started: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
}

struct MetricsAndValidationPlugin {
    counters: Arc<Counters>,
}

impl Plugin for MetricsAndValidationPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(JobStart, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.started.fetch_add(1, Ordering::Relaxed);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobComplete, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.completed.fetch_add(1, Ordering::Relaxed);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobFail, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.failed.fetch_add(1, Ordering::Relaxed);
            }
        });

        hooks.on(BeforeJobRun, async |ctx| {
            if ctx
                .payload
                .get("skip")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return HookResult::Skip;
            }

            HookResult::Continue
        });
    }
}
```

See the repository examples in `examples/hooks.rs` and `examples/hooks/*.rs`
for a runnable version with logging, metrics, validation, and sample jobs.
