# Examples Tour

The repository includes runnable examples that show how the worker APIs fit
together in real programs. This page tours each file under `examples/` and
calls out what to learn from it.

Most examples connect to PostgreSQL, initialize a worker, add one or more jobs,
and then run the worker. Several examples fall back to
`postgres://postgres:root@localhost:5432`; others read `DATABASE_URL` first.

## Basic worker flow

### `examples/simple.rs`

Start here if you want the smallest complete worker shape:

1. Define a serializable task payload.
2. Implement `TaskHandler` with an `IDENTIFIER`.
3. Build `WorkerOptions`, register the task, attach a PostgreSQL pool, and call
   `init()`.
4. Use `worker.create_utils()` to enqueue work.
5. Call `worker.run()` to process jobs continuously.

The task intentionally fails most of the time so you can observe retry
behavior:

```rust,ignore
impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Hello {} !", self.message);

        if rand::rng().random_range(0..100) < 70 {
            return Err("Failed".to_string());
        }

        Ok(())
    }
}
```

It also schedules its first job for 10 seconds in the future with
`JobSpecBuilder::run_at`.

### `examples/run_once.rs`

Use this example when you want a bounded worker pass instead of a long-running
process. It schedules 10 `SayHello` jobs, then calls:

```rust,ignore
worker.run_once().await.unwrap();
```

This is useful for learning the difference between continuous workers and a
single processing pass.

## Sharing application state

### `examples/app_state.rs`

This example shows how to attach application-owned state to the worker and read
it from task handlers.

`AppState` wraps an `Arc<AtomicUsize>`, is installed with
`WorkerOptions::add_extension`, and is retrieved from the task context:

```rust,ignore
let app_state = ctx.get_ext::<AppState>().unwrap();
let run_count = app_state.increment_run_count();
println!("Run count: {run_count}");
```

The file also combines normal enqueued jobs with a cron definition using
`Cron::every_minute::<ShowRunCount>().fill(CrontabFill::hours(1))`, so it is a
compact example of state plus recurring jobs.

## Scheduling work from a handler

### `examples/context_helpers.rs`

This example demonstrates `WorkerContextExt` helper methods inside a running
task. `SendWs` represents an initial operation, then schedules a `CheckWs`
follow-up job from inside `run`:

```rust,ignore
ctx.add_job(
    CheckWs {
        request_id: self.request_id.clone(),
    },
    JobSpecBuilder::new()
        .run_at(Utc::now() + Duration::seconds(10))
        .build(),
)
.await?;
```

Use this pattern for workflows where one job needs to create the next step after
it succeeds.

## Adding jobs in batches

### `examples/batch_add_jobs.rs`

This file compares two batch insertion APIs:

- `add_jobs::<SendEmail>(&emails)` for many jobs with the same typed payload.
- `add_raw_jobs(&mixed_jobs)` for heterogeneous jobs where each item provides an
  identifier, JSON payload, and `JobSpec`.

The raw batch includes both `send_email` and `process_payment` jobs, and uses
`JobSpecBuilder::priority(-10)` for one urgent payment job:

```rust,ignore
let mixed_jobs = vec![
    RawJobSpec {
        identifier: "send_email".into(),
        payload: json!({ "to": "dave@example.com", "subject": "Notification" }),
        spec: JobSpec::default(),
    },
    RawJobSpec {
        identifier: "process_payment".into(),
        payload: json!({ "user_id": 123, "amount": 50 }),
        spec: JobSpecBuilder::new().priority(-10).build(),
    },
];
```

Use typed batches when all payloads share one handler type. Use raw batches when
you need one database insert for multiple task identifiers.

## Cron jobs

### `examples/crontab.rs`

This example focuses on recurring jobs. It registers two task types and defines
a cron entry for `SayHello`:

```rust,ignore
Cron::every_n_minutes::<SayHello>(2)
    .unwrap()
    .fill(CrontabFill::minutes(10))
    .job_key("say_hello_dedupe")
    .job_key_mode(CronJobKeyMode::PreserveRunAt)
    .payload(SayHello {
        message: "Crontab".to_string(),
    })
    .unwrap()
```

The example shows how to set:

- the interval with `every_n_minutes`;
- backfill with `CrontabFill`;
- a `job_key` for deduplication;
- `CronJobKeyMode::PreserveRunAt`;
- a typed payload for the recurring job.

See also [Cron Jobs](../guides/cron.md) for a broader explanation of recurring
work.

## Plugins and hooks

### `examples/hooks.rs`

This is the entry point for the hooks example. It wires together:

- `ProcessData`, the task being processed;
- `MetricsPlugin`, which observes worker and job lifecycle events;
- `ValidationPlugin`, which can continue, skip, or fail a job before it runs;
- `logging::enable_logs`, a small tracing setup.

It enqueues four jobs so you can see each path: normal processing, skip,
forced failure, and another normal job.

### `examples/hooks/process_data.rs`

This file defines the payload used by the hooks example:

```rust,ignore
pub(super) struct ProcessData {
    pub(super) value: i32,
    #[serde(default)]
    pub(super) skip: bool,
    #[serde(default)]
    pub(super) force_fail: bool,
}
```

The handler itself only prints the value. The interesting behavior comes from
plugins inspecting `skip` and `force_fail`.

### `examples/hooks/validation.rs`

`ValidationPlugin` registers a `BeforeJobRun` hook. It reads the raw JSON
payload from the hook context:

- when `skip` is true, it returns `HookResult::Skip`;
- when `force_fail` is true, it returns `HookResult::Fail(...)`;
- otherwise it returns `HookResult::Continue`.

This is the file to study when a plugin needs to make a pre-run decision.

### `examples/hooks/metrics.rs`

`MetricsPlugin` registers lifecycle hooks for:

- `WorkerStart`;
- `WorkerShutdown`;
- `JobStart`;
- `JobComplete`;
- `JobFail`.

It keeps atomic counters for started, completed, and failed jobs, then prints
final counts during shutdown.

### `examples/hooks/logging.rs`

This helper installs a `tracing_subscriber` registry with a debug-level filter
and `sqlx=warn`. It is intentionally small so the hook examples can focus on
plugin behavior.

## Local queue behavior

### `examples/local_queue.rs`

This example shows how to enable and observe local queue batching. It configures
the worker with:

```rust,ignore
.local_queue(
    LocalQueueConfig::default()
        .with_size(50)
        .with_ttl(Duration::from_secs(60))
        .with_refetch_delay(
            RefetchDelayConfig::default()
                .with_duration(Duration::from_millis(100))
                .with_threshold(5)
                .with_max_abort_threshold(20),
        ),
)
```

The local plugin logs queue-specific hooks such as initialization, mode changes,
jobs fetched, jobs returned, refetch delay start, delay abort, and delay expiry.
The example enqueues 20 short `ProcessItem` jobs so you can watch batch fetching
and refetch-delay behavior in logs.

## Worker utilities in a web server

The `sendable_worker` example is split across several files. Together they show
that a worker can run beside an HTTP server while `WorkerUtils` is shared with
request handlers.

### `examples/sendable_worker.rs`

This is the example entry point. It:

- builds a worker with `Worker::options()`;
- registers `ExampleTask` and `DatabaseTask`;
- wraps `worker.create_utils()` in `Arc`;
- binds a TCP listener on `127.0.0.1:3000`;
- spawns the worker with `tokio::spawn`;
- spawns a simple HTTP accept loop;
- starts a second worker against the same database URL.

The application waits with `tokio::select!` for a worker task, server task,
secondary worker task, or Ctrl+C.

### `examples/sendable_worker/tasks.rs`

This file defines the two task handlers used by the HTTP example:

- `ExampleTask` logs a name and value, sleeps for 100 milliseconds, and
  succeeds.
- `DatabaseTask` uses `ctx.pg_pool()` to run
  `SELECT COUNT(*) FROM graphile_worker.jobs` and logs the current job count.

Study this file when a task needs access to the worker's PostgreSQL pool.

### `examples/sendable_worker/http.rs`

This file contains the request dispatcher. It reads the HTTP request line,
extracts the method and path, and routes:

- `GET /` and `GET /health`;
- `POST /schedule/example?...`;
- `POST /schedule/database?...`;
- everything else to `404 Not Found`.

It is deliberately minimal HTTP parsing, enough to demonstrate sharing
`WorkerUtils` with request handlers.

### `examples/sendable_worker/http/schedule.rs`

This file schedules jobs from HTTP routes.

`schedule_example_task` parses `name` and `value`, then calls
`utils.add_job(ExampleTask { name, value }, Default::default())`.

`schedule_database_task` requires a `query` parameter and uses a richer
`JobSpecBuilder`:

```rust,ignore
let job_spec = JobSpecBuilder::new()
    .priority(-10)
    .run_at(chrono::Utc::now() + chrono::Duration::seconds(10))
    .job_key(format!("db_task_{}", chrono::Utc::now().timestamp()))
    .build();
```

This demonstrates scheduling from request input with priority, delay, and job
key options.

### `examples/sendable_worker/http/query.rs`

This helper parses query parameters into a `HashMap<String, String>` and applies
a small percent-decoding function for the encoded characters used by the
example routes.

### `examples/sendable_worker/http/response.rs`

This helper writes a plain-text HTTP response to a `TcpStream`, including status
line, content length, content type, and body.
