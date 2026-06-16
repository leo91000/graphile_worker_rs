# Cron Jobs

Cron jobs schedule registered tasks from cron-like definitions. They are useful
for recurring work such as cleanup, reporting, reconciliation, and periodic
synchronization.

Cron entries can be added with typed Rust builders or with crontab text:

- `WorkerOptions::with_cron(...)` accepts one cron entry. It accepts typed
  builders, raw `Crontab` values, and crontab text.
- `WorkerOptions::with_crons(...)` accepts multiple typed or raw `Crontab`
  values.
- `WorkerOptions::with_crontab(...)` accepts crontab text, but is deprecated in
  favor of `with_cron(...)`.

## Typed cron builders

Use `Cron` when the schedule belongs to a Rust `TaskHandler`. The task
identifier comes from `T::IDENTIFIER`, so it stays aligned with the job you
registered with `define_job`.

```rust,ignore
use graphile_worker::{
    Cron, CrontabFill, IntoTaskHandlerResult, TaskHandler, WorkerContext,
    WorkerOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendDigest {
    account_id: i64,
}

impl TaskHandler for SendDigest {
    const IDENTIFIER: &'static str = "send_digest";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        // Send the digest.
    }
}

let worker = WorkerOptions::default()
    .define_job::<SendDigest>()
    .with_cron(
        Cron::daily_at::<SendDigest>(8, 0)?
            .id("send_digest_morning")
            .fill(CrontabFill::hours(1))
            .payload(SendDigest { account_id: 42 })?,
    )
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Typed schedules are built with:

```rust,ignore
Cron::every_minute::<MyTask>()
Cron::every_n_minutes::<MyTask>(5)?
Cron::hourly_at::<MyTask>(15)?
Cron::daily_at::<MyTask>(8, 0)?
Cron::weekly_on::<MyTask>(chrono::Weekday::Mon, 8, 0)?
Cron::monthly_on::<MyTask>(1, 8, 0)?
Cron::yearly_on::<MyTask>(1, 1, 8, 0)?
```

The typed constructors validate field ranges. Minutes are `0..=59`, hours are
`0..=23`, days of month are `1..=31`, and months are `1..=12`.

Builder options include:

```rust,ignore
Cron::every_n_minutes::<SendDigest>(10)?
    .id("send_digest_every_10_minutes")
    .fill(CrontabFill::minutes(30))
    .max_attempts(5)
    .queue("digest")
    .priority(-10)
    .job_key("send_digest_dedupe")
    .job_key_mode(graphile_worker::CronJobKeyMode::PreserveRunAt)
    .payload(SendDigest { account_id: 42 })?;
```

Use `payload(...)` for a typed payload that serializes as JSON. Use
`payload_value(...)` when you already have a `serde_json::Value`.

## Multiple cron entries

Use `with_crons` for a collection of typed builders or `Crontab` values:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendDigest>()
    .define_job::<SyncMetrics>()
    .with_crons([
        Cron::daily_at::<SendDigest>(8, 0)?.build(),
        Cron::every_n_minutes::<SyncMetrics>(15)?.build(),
    ])
    .pg_pool(pg_pool)
    .init()
    .await?;
```

## Crontab text

`with_cron` also accepts crontab text and returns a `Result` because the string
is parsed at runtime.

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendDigest>()
    .with_cron(
        r#"
        # Run at 08:00 UTC every day.
        0 8 * * * send_digest ?id=send_digest_morning&fill=1h {account_id:42}
        "#,
    )?
    .pg_pool(pg_pool)
    .init()
    .await?;
```

The text format is:

```text
* * * * * task_identifier ?options {payload}
```

The five schedule fields are UTC minute, UTC hour, UTC day of month, UTC month,
and UTC day of week. Days of week use `0..=6`.

Supported schedule values are:

- `*` for every valid value.
- `*/n` for every value divisible by `n`.
- `n` for one explicit value.
- `a-b` for an inclusive range.
- Comma-separated combinations such as `0,15,30,45` or `4,10-15`.

Examples:

```text
# Every minute.
* * * * * tick

# Every 4 hours at minute 0.
0 */4 * * * rollup

# Mondays at 04:30 UTC.
30 4 * * 1 send_weekly_email

# More than one value in a field.
30 4,10-15 * * 1 send_weekly_email
```

Task identifiers must start with a letter or underscore. After that they may
contain letters, numbers, colon, underscore, or hyphen.

## Options

Crontab text options use query-string syntax and must start with `?`.

```text
0 8 * * * send_digest ?id=send_digest_morning&fill=1h&max=5&queue=digest&priority=10
```

Supported options are:

- `id`: stable identifier for this cron entry. By default the task identifier is
  used. Set this when the same task has more than one schedule.
- `fill`: backfill duration for missed executions.
- `max`: overrides the job's maximum attempts.
- `queue`: schedules the job in a named queue.
- `priority`: overrides the job priority.
- `job_key`: sets a job key for deduplication.
- `job_key_mode`: controls an existing unlocked job with the same key. Supported
  serialized values are `replace` and `preserve_run_at`.

Changing a cron entry identifier can cause the worker to treat it as a new cron
entry. Prefer setting a stable `id` when a task has multiple cron schedules.

## Payloads

Cron jobs receive a JSON object payload. The worker adds a `_cron` object to the
payload for each scheduled job:

```json
{
  "_cron": {
    "ts": "2026-06-16T08:00:00",
    "backfilled": false
  }
}
```

`ts` is the timestamp when the cron execution was due, and `backfilled` is
`true` when the job was scheduled as a backfill.

With typed builders, set the payload with `payload(...)` or
`payload_value(...)`. With crontab text, write a JSON5 object after the options:

```text
0 8 * * * send_digest ?id=send_digest_morning {account_id:42, urgent:false}
```

The parsed payload is merged with the default cron payload properties. The text
payload must start with `{` and stay on one line.

## Backfill and fill

By default, cron entries do not backfill missed executions. Add `fill` to ask
the worker to schedule missed runs from a recent time window when it starts or
recovers a known cron entry.

Typed builder:

```rust,ignore
Cron::every_n_minutes::<SyncMetrics>(10)?
    .id("sync_metrics")
    .fill(CrontabFill::minutes(30))
```

Crontab text:

```text
*/10 * * * * sync_metrics ?id=sync_metrics&fill=30m
```

Fill durations are made from time units:

- `s`: seconds
- `m`: minutes
- `h`: hours
- `d`: days
- `w`: weeks

Units may be combined in order, for example `1w2d3h30m`. In Rust, use
`CrontabFill::seconds`, `minutes`, `hours`, `days`, `weeks`, or
`CrontabFill::new(w, d, h, m, s)`.

Backfill only applies to cron entries that were already known to the worker.
When a cron entry is first registered, the worker records it as known but does
not immediately create old jobs for it. Tests cover this behavior by asserting
that a newly registered cron entry has no `last_execution` and creates no
backfill jobs.

When a known cron entry has a previous execution timestamp, the worker can catch
up missed matching schedule times inside the configured fill window. A four-hour
cron with `fill=1d`, for example, can create the missed four-hour executions
from the last day. Larger fill windows can schedule more jobs and take longer at
startup.

## Runtime behavior

The cron runner checks schedule times and inserts regular jobs for matching
entries. If time advances by multiple matching intervals, the runner catches up
by scheduling the missed ticks it observes. Jobs are then processed by the same
registered task handlers as jobs added through the regular job APIs.
