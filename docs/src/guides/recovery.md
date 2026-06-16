# Worker Recovery

Worker recovery is an opt-in safety mechanism for jobs that remain locked after a
worker stops unexpectedly. It is useful when a process crashes, is aborted, loses
database connectivity, or is terminated by an orchestrator before it can release
its in-flight jobs.

When recovery is enabled, workers write heartbeat rows to PostgreSQL. A sweeper
then looks for workers whose heartbeat is stale and returns their locked jobs to
the queue.

## Enabling Recovery

Recovery is disabled by default. You can enable it with `WorkerRecoveryConfig`:

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

The convenience setters on `WorkerOptions` also enable recovery:

```rust,ignore
use graphile_worker::WorkerOptions;
use std::time::Duration;

let worker = WorkerOptions::default()
    .heartbeat_interval(Duration::from_secs(30))
    .sweep_interval(Duration::from_secs(60))
    .sweep_threshold(Duration::from_secs(300))
    .recovery_delay(Duration::from_secs(30))
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

## Configuration

`WorkerRecoveryConfig::default()` uses these values:

| Option | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | Whether heartbeat registration and sweeping are active. |
| `heartbeat_interval` | 30 seconds | How often an enabled worker updates its heartbeat row. |
| `sweep_interval` | 60 seconds | How often the background sweeper checks for inactive workers. |
| `sweep_threshold` | 5 minutes | How old a heartbeat must be before the worker is considered inactive. |
| `recovery_delay` | 30 seconds | Delay before recovered jobs are eligible to run again. |
| `resilient_sweep_threshold_multiplier` | `3` | Multiplier applied to `sweep_threshold` for workers holding resilient jobs. |
| `resilient_job_flags` | `["infrastructure_resilient"]` | Job flags that activate the extended threshold. |

Each builder method on `WorkerRecoveryConfig` stores the new value and enables
recovery, except `.enabled(false)`, which explicitly disables it again.

## Heartbeats

An enabled worker registers itself in the private workers table and refreshes
`last_heartbeat_at` at the configured `heartbeat_interval`.

You can inspect registered workers with `WorkerUtils::list_active_workers`.
The stale state is calculated from the threshold you pass to that method:

```rust,ignore
use std::time::Duration;

let workers = utils
    .list_active_workers(Duration::from_secs(60))
    .await?;

for worker in workers {
    println!(
        "worker={} stale={} started_at={} last_heartbeat_at={}",
        worker.worker_id,
        worker.is_stale,
        worker.started_at,
        worker.last_heartbeat_at
    );
}
```

## Sweeping Stale Workers

The background sweeper runs at `sweep_interval`. During a sweep it:

- takes a transaction-scoped PostgreSQL advisory lock, so concurrent sweepers do
  not recover the same jobs twice;
- finds workers whose heartbeat is older than `sweep_threshold`;
- also finds orphan job locks whose `locked_by` worker is no longer registered;
- unlocks the jobs held by those workers;
- decrements the recovered jobs' attempt count back down;
- releases queue locks for queued jobs;
- moves `run_at` forward by `recovery_delay`;
- records `Job recovered after worker interruption` as the recovered job error.

If another worker already holds the sweep lock, the sweep exits without
recovering jobs.

## Manual Sweeps

Use `WorkerUtils::sweep_stale_workers` when you want an operator command, admin
endpoint, or scheduled task to run recovery directly.

```rust,ignore
use graphile_worker::{SweepStaleWorkersOptions, WorkerUtils};
use std::time::Duration;

let utils = WorkerUtils::new(database, "graphile_worker".to_string());

let result = utils
    .sweep_stale_workers(SweepStaleWorkersOptions {
        sweep_threshold: Some(Duration::from_secs(60)),
        recovery_delay: Some(Duration::from_secs(30)),
        dry_run: false,
    })
    .await?;

println!(
    "recovered {} jobs from {:?}",
    result.recovered_count,
    result.worker_ids
);
```

`SweepStaleWorkersOptions` can override `sweep_threshold` and `recovery_delay`
for a single sweep. Leave either field as `None` to use the recovery config
defaults. Set `dry_run: true` to return the worker IDs that would be considered
dead without recovering jobs or deleting stale worker rows.

When you need custom resilient settings for a manual sweep, pass a
`WorkerRecoveryConfig` explicitly:

```rust,ignore
use graphile_worker::{SweepStaleWorkersOptions, WorkerRecoveryConfig};
use std::time::Duration;

let config = WorkerRecoveryConfig::default()
    .sweep_threshold(Duration::from_secs(60))
    .resilient_sweep_threshold_multiplier(3);

let result = utils
    .sweep_stale_workers_with_config(&config, SweepStaleWorkersOptions {
        recovery_delay: Some(Duration::from_secs(30)),
        dry_run: false,
        ..Default::default()
    })
    .await?;
```

## Resilient Jobs

Some jobs are expected to run for a long time. Recovery supports a resilient flag
so those jobs are not reclaimed as quickly as ordinary work.

By default, a job is resilient when its flags include
`"infrastructure_resilient": true`. Workers holding resilient jobs use:

```text
effective threshold = sweep_threshold * resilient_sweep_threshold_multiplier
```

The default multiplier is `3`.

```rust,ignore
use graphile_worker::{JobSpec, INFRASTRUCTURE_RESILIENT_FLAG};

let job = utils
    .add_job(
        LongRunningJob { id: 1 },
        JobSpec::builder()
            .flags(vec![INFRASTRUCTURE_RESILIENT_FLAG.to_string()])
            .build(),
    )
    .await?;
```

You can configure a different flag list:

```rust,ignore
use graphile_worker::WorkerRecoveryConfig;

let recovery = WorkerRecoveryConfig::default()
    .resilient_job_flags(vec!["custom_resilient".to_string()]);
```

A configured flag must be present and truthy on the job for the extended
threshold to apply.

## Recovery Hooks

Recovery hooks let your application decide what to do with each job recovered
from a crashed worker. The hook receives the job, the recovering worker ID, the
previous worker ID, and the interruption reason.

```rust,ignore
use graphile_worker::{
    FailureReason, HookRegistry, JobRecovery, JobRecoveryResult, SweepStaleWorkersOptions,
    WorkerUtils,
};
use std::sync::Arc;

let mut hooks = HookRegistry::new();

hooks.on(JobRecovery, |ctx| async move {
    if ctx.reason == FailureReason::WorkerCrashed {
        JobRecoveryResult::Default
    } else {
        JobRecoveryResult::Skip
    }
});

let utils = WorkerUtils::new(database, "graphile_worker".to_string())
    .with_hooks(Arc::new(hooks));

let result = utils
    .sweep_stale_workers(SweepStaleWorkersOptions::default())
    .await?;
```

The hook result controls recovery for that job:

| Result | Effect |
| --- | --- |
| `JobRecoveryResult::Default` | Unlock the job, decrement attempts, and delay it by `recovery_delay`. |
| `JobRecoveryResult::Reschedule { run_at, attempts }` | Unlock the job, set `run_at`, and optionally replace `attempts`. |
| `JobRecoveryResult::FailWithBackoff` | Fail the job with the normal retry backoff and `WorkerCrashed` as the error. |
| `JobRecoveryResult::Skip` | Leave the job locked and do not count it as recovered. |

After a hook handles a job with any result except `Skip`, the normal job
interrupted hook is emitted with `FailureReason::WorkerCrashed`. After a sweep
finishes, worker recovery hooks are emitted with the recovered worker IDs and
the recovered job count.
