# Scheduling Model

Graphile Worker RS schedules work by inserting rows into PostgreSQL. A job
becomes runnable when it is available, unlocked, below its retry limit, and its
`run_at` timestamp is due.

Most Rust applications schedule jobs through `WorkerUtils`:

```rust,ignore
let utils = worker.create_utils();

utils.add_job(
    SendEmail {
        to: "user@example.com".to_string(),
        subject: "Welcome".to_string(),
        body: "Thanks for signing up.".to_string(),
    },
    JobSpec::default(),
).await?;
```

Use `add_raw_job` when the task identifier is only known at runtime:

```rust,ignore
utils.add_raw_job(
    "send_email",
    serde_json::json!({
        "to": "user@example.com",
        "subject": "Welcome"
    }),
    JobSpec::default(),
).await?;
```

## JobSpec

`JobSpec` contains the scheduling metadata sent to the database:

```rust,ignore
use chrono::Utc;
use graphile_worker::{JobKeyMode, JobSpecBuilder};

let spec = JobSpecBuilder::new()
    .queue_name("user:123")
    .run_at(Utc::now() + chrono::Duration::minutes(5))
    .max_attempts(5)
    .job_key("welcome-email:123")
    .job_key_mode(JobKeyMode::Replace)
    .priority(-10)
    .flags(vec!["email".to_string()])
    .build();
```

The database fills omitted values:

| Field | Meaning | Default |
| --- | --- | --- |
| `queue_name` | Optional queue used to serialize related jobs. | No queue |
| `run_at` | Earliest timestamp at which the job may run. | `now()` |
| `max_attempts` | Maximum attempts before the job is permanently unavailable. | `25` |
| `job_key` | Optional deduplication key. | No key |
| `job_key_mode` | How an existing keyed job is handled. | `Replace` |
| `priority` | Sort key for runnable jobs. Lower numbers run first. | `0` |
| `flags` | Optional flags stored on the job. | No flags |

When `WorkerUtils` is configured with `with_use_local_time(true)`, Rust passes
the application's current UTC time for omitted `run_at` values. Otherwise the
database uses PostgreSQL `now()`.

## Immediate And Future Jobs

`JobSpec::default()` schedules a job immediately. To delay a job, set `run_at`:

```rust,ignore
use chrono::Utc;
use graphile_worker::JobSpecBuilder;

let spec = JobSpecBuilder::new()
    .run_at(Utc::now() + chrono::Duration::hours(1))
    .build();

utils.add_job(SendEmail { /* ... */ }, spec).await?;
```

Workers only fetch jobs whose `run_at` is less than or equal to the current
time used by the worker query.

## Priority

Runnable jobs are selected in ascending priority order, then ascending `run_at`.
The default priority is `0`, so negative values run before default-priority jobs
and positive values run later.

```rust,ignore
let urgent = JobSpecBuilder::new()
    .priority(-10)
    .build();

let normal = JobSpec::default();
```

Priority is not a separate queue. It only affects ordering among jobs that are
otherwise available to the worker.

## Job Keys

A `job_key` deduplicates jobs. Adding another available job with the same key
updates the existing row instead of creating another row.

```rust,ignore
use graphile_worker::{JobKeyMode, JobSpecBuilder};

let spec = JobSpecBuilder::new()
    .job_key("sync-user:123")
    .job_key_mode(JobKeyMode::Replace)
    .build();

utils.add_job(SyncUser { user_id: 123 }, spec).await?;
```

The supported `JobKeyMode` values are:

| Mode | Behavior |
| --- | --- |
| `Replace` | Replace the existing available job's queue, task, payload, `run_at`, `max_attempts`, priority, and flags. Reset attempts and clear the last error. |
| `PreserveRunAt` | Replace the job, but keep the existing `run_at` when the existing job has not been attempted yet. |
| `UnsafeDedupe` | If the key already exists, only increment the revision and update `updated_at`; the existing payload and timing are left in place. |

For a keyed job that already exists but is no longer available, the database
clears the old key and marks that old row as fully attempted before inserting
the replacement.

Batch scheduling supports `Replace` and `PreserveRunAt`. `UnsafeDedupe` is
rejected for `add_jobs` and `add_raw_jobs`. In a batch, if any job uses
`PreserveRunAt`, the preserve-run-at behavior is applied to keyed conflicts in
that batch.

## Queues

`queue_name` groups jobs that must not run in parallel. Jobs with the same queue
share a row in the private job queue table, and the worker locks that queue when
it fetches a queued job.

```rust,ignore
let spec = JobSpecBuilder::new()
    .queue_name("account:123")
    .build();

utils.add_job(UpdateAccount { id: 123 }, spec).await?;
```

Use queues for per-user, per-account, or per-resource workflows where ordering
or mutual exclusion matters. Jobs without a queue are not serialized by this
mechanism.

## Batch Scheduling

Use `add_jobs` or `add_raw_jobs` to insert many jobs in one database round trip:

```rust,ignore
let spec = JobSpec::default();

utils.add_jobs::<SendEmail>(&[
    (SendEmail { /* ... */ }, &spec),
    (SendEmail { /* ... */ }, &spec),
]).await?;
```

For batch task handlers, `add_batch_job` stores a JSON array in a single job.
When a keyed batch job is replaced and both the existing and new payloads are
arrays, the database appends the new array to the existing array.

## Cron

Cron schedules are configured on `WorkerOptions`, not by manually looping in
application code. The typed API builds cron definitions in Rust:

```rust,ignore
use graphile_worker::{Cron, CrontabFill, WorkerOptions};

let worker = WorkerOptions::default()
    .define_job::<SendDailyReport>()
    .with_cron(
        Cron::daily_at::<SendDailyReport>(8, 0)?
            .fill(CrontabFill::hours(1)),
    )
    .init()
    .await?;
```

Crontab text is also supported:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendDailyReport>()
    .with_cron("0 8 * * * send_daily_report")?
    .init()
    .await?;
```

On each cron tick, the runner checks which crontabs match the current local
timestamp and schedules matching jobs through the same database `add_job`
function used by one-off jobs. Cron jobs can carry payload, queue, `run_at`,
`max_attempts`, priority, job key, and job key mode metadata from the crontab
definition. If the payload is a JSON object, the runner adds a `_cron` object
with the scheduled timestamp and whether the job was backfilled.

The runner records known crontabs and their last execution timestamp so a cron
tick is only scheduled once for a given crontab execution.

## Retry Metadata

The worker increments `attempts` when it locks a job to run it. A job remains
available only while `attempts < max_attempts`.

When a job fails, the failure query stores `last_error`, unlocks the job, and
sets a future `run_at` using exponential backoff:

```text
greatest(now(), run_at) + exp(least(attempts, 10)) seconds
```

This means retry delay grows with the attempt count and is capped by using at
most attempt `10` in the exponent. Once `attempts` reaches `max_attempts`, the
job is no longer available for normal fetching.

Existing jobs can be changed with management utilities such as rescheduling.
`RescheduleJobOptions` can update `run_at`, `priority`, `attempts`, and
`max_attempts` for selected job ids.
