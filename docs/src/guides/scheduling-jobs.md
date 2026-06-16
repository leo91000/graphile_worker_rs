# Scheduling Jobs

Jobs can be scheduled either from SQL or from Rust. Both paths write to the
same Graphile Worker tables and use the same job options: task identifier,
JSON payload, queue name, scheduled time, retry limit, job key, priority, and
flags.

Use SQL when you are already inside a database migration, trigger, or stored
procedure. Use `WorkerUtils` when scheduling from Rust application code.

## Scheduling From SQL

The worker schema exposes an `add_job` function. The default schema is
`graphile_worker`, so a minimal insert looks like this:

```sql
select * from graphile_worker.add_job(
    identifier => 'send_email',
    payload => '{"to":"user@example.com"}'::json
);
```

The Rust scheduler calls the same function with these named arguments:

```sql
select * from graphile_worker.add_job(
    identifier => 'send_email',
    payload => '{"to":"user@example.com"}'::json,
    queue_name => 'mailers',
    run_at => now() + interval '5 minutes',
    max_attempts => 10,
    job_key => 'send_email:user@example.com',
    priority => 5,
    flags => array['transactional'],
    job_key_mode => 'replace'
);
```

`identifier` must match a registered task identifier in your worker process.
`payload` is passed to the task handler as JSON.

## Scheduling From Rust

Create `WorkerUtils` from a running worker when you want it to share the
worker's database, schema, hooks, task details, and time configuration:

```rust,ignore
let utils = worker.create_utils();
```

For a typed task, pass the task payload and a `JobSpec`. The task identifier
comes from `TaskHandler::IDENTIFIER`, and the payload is serialized with
`serde_json`.

```rust,ignore
use graphile_worker::{JobSpec, TaskHandler};

let job = utils
    .add_job(
        SendEmail {
            to: "user@example.com".to_string(),
            subject: "Welcome".to_string(),
        },
        JobSpec::default(),
    )
    .await?;
```

For dynamic scheduling, use a raw identifier and any serializable payload:

```rust,ignore
use graphile_worker::JobSpec;
use serde_json::json;

let job = utils
    .add_raw_job(
        "send_email",
        json!({
            "to": "user@example.com",
            "subject": "Welcome"
        }),
        JobSpec::default(),
    )
    .await?;
```

Raw jobs are useful when the caller does not know the Rust task type at compile
time. Typed jobs are preferred when the task type is available because the
identifier and payload type stay tied to the `TaskHandler` implementation.

## Job Options

`JobSpec` controls how the job is inserted:

```rust,ignore
use chrono::{Duration, Utc};
use graphile_worker::{JobKeyMode, JobSpecBuilder};

let spec = JobSpecBuilder::new()
    .queue_name("mailers")
    .run_at(Utc::now() + Duration::minutes(5))
    .max_attempts(10)
    .job_key("send_email:user@example.com")
    .job_key_mode(JobKeyMode::Replace)
    .priority(5)
    .flags(vec!["transactional".to_string()])
    .build();
```

All `JobSpec` fields are optional. `JobSpec::default()` or
`JobSpec::new()` schedules with no explicit options.

| Field | Effect |
| --- | --- |
| `queue_name` | Assigns the job to a named queue. |
| `run_at` | Delays execution until the given UTC timestamp. |
| `max_attempts` | Sets the maximum number of attempts before the job is permanently failed. |
| `job_key` | Deduplicates scheduled jobs with the same key. |
| `job_key_mode` | Controls how an existing keyed job is handled. |
| `priority` | Sets job priority. Lower numbers run sooner. |
| `flags` | Stores string flags on the job for worker features and custom conventions. |

When `run_at` is not supplied, the database function schedules the job using
its default time behavior. If the worker is configured to use local application
time, `WorkerUtils` supplies `Utc::now()` for jobs without an explicit `run_at`.

## Job Keys

Set `job_key` when multiple scheduling attempts should refer to the same
logical job.

```rust,ignore
use graphile_worker::{JobKeyMode, JobSpecBuilder};

let spec = JobSpecBuilder::new()
    .job_key("recalculate-account:42")
    .job_key_mode(JobKeyMode::Replace)
    .build();

utils.add_job(RecalculateAccount { account_id: 42 }, spec).await?;
```

The supported modes are:

| Mode | Behavior visible from the scheduler |
| --- | --- |
| `JobKeyMode::Replace` | Replaces the existing keyed job data. This is the default mode when a key is used. |
| `JobKeyMode::PreserveRunAt` | Updates the keyed job but keeps its existing `run_at`. |
| `JobKeyMode::UnsafeDedupe` | Deduplicates without replacing the existing `run_at`; supported for single-job scheduling only. |

When a keyed job is updated, the stored job is reused and its revision is
incremented. If a keyed job is locked while a replacement is scheduled, the
implementation retires the locked row for normal failure handling and keeps a
single replacement row with the key.

## Bulk Scheduling

Use `add_jobs` to schedule many jobs of the same task type in one database
round trip:

```rust,ignore
use graphile_worker::JobSpecBuilder;

let urgent = JobSpecBuilder::new().priority(-10).build();
let normal = JobSpec::default();

let jobs = vec![
    (SendEmail { to: "a@example.com".to_string() }, &urgent),
    (SendEmail { to: "b@example.com".to_string() }, &normal),
];

let added = utils.add_jobs::<SendEmail>(&jobs).await?;
```

Each item can reference its own `JobSpec`, or many items can share the same
spec.

Use `add_raw_jobs` when a single bulk insert needs heterogeneous task
identifiers:

```rust,ignore
use graphile_worker::{JobSpec, RawJobSpec};
use serde_json::json;

let jobs = vec![
    RawJobSpec {
        identifier: "send_email".into(),
        payload: json!({ "to": "a@example.com" }),
        spec: JobSpec::default(),
    },
    RawJobSpec {
        identifier: "refresh_search_index".into(),
        payload: json!({ "model": "account", "id": 42 }),
        spec: JobSpec::default(),
    },
];

let added = utils.add_raw_jobs(&jobs).await?;
```

Empty bulk inputs return an empty `Vec` and do not insert jobs.

Bulk scheduling has two job-key limitations:

- `JobKeyMode::UnsafeDedupe` is rejected for `add_jobs` and `add_raw_jobs`.
- If any job in the batch uses `JobKeyMode::PreserveRunAt`, the batch call
  applies the preserve-run-at setting to the underlying database operation. Use
  individual `add_job` or `add_raw_job` calls when each job needs independent
  key-mode behavior.

## Attempts, Priority, And Flags

`max_attempts` controls the retry limit for the scheduled job. Failed jobs are
retried by the worker until their attempt count reaches this limit.

`priority` is stored as an integer. Jobs are fetched in ascending priority
order, so lower values, including negative values, run before the default
priority `0`.

`flags` are stored as an array of strings. The scheduler does not interpret
custom flag names while adding jobs; it passes them through to the database.

For changing jobs after they have been scheduled, see
[Job Management](./job-management.md).
