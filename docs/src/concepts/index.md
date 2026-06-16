# Core Concepts

Graphile Worker RS is a PostgreSQL-backed job queue for Rust applications. The
core mental model is:

1. Your application describes work as typed tasks.
2. It adds jobs for those tasks into PostgreSQL.
3. A worker fetches runnable jobs, executes the matching task handler, and
   records the result back in PostgreSQL.

The database is the coordination point. Jobs, task metadata, queue state,
worker state, retry metadata, and scheduling data live in the Graphile Worker
schema, which defaults to `graphile_worker`.

## The Main Pieces

### Tasks

A task is the Rust type that defines what a job does. It implements
`TaskHandler`, provides a stable task identifier, accepts a serializable payload,
and contains the async `run` method for the work.

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending email to {}", self.to);
        Ok::<(), String>(())
    }
}
```

Register task handlers on `WorkerOptions` before starting the worker:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

For more detail, see [Tasks and Payloads](tasks.md).

### Jobs

A job is a stored request to run a task. It includes the task identifier,
payload, state, and execution metadata. Job specifications can also carry
options such as priority, retry behavior, scheduling information, and queue
selection.

Jobs are stored in PostgreSQL, so application processes can enqueue work and
worker processes can execute it independently. `WorkerUtils` provides utility
operations for managing jobs, such as adding, removing, and rescheduling them.

### Workers

A worker is the runtime process that polls PostgreSQL, locks runnable jobs, and
executes registered handlers with the configured concurrency. Graphile Worker RS
uses PostgreSQL features such as `SKIP LOCKED` for efficient job fetching and
`LISTEN`/`NOTIFY` for low-latency wakeups.

```rust,ignore
graphile_worker::WorkerOptions::default()
    .concurrency(5)
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?
    .run()
    .await?;
```

Workers also own lifecycle concerns such as graceful shutdown. Optional recovery
settings can record worker heartbeats and recover jobs locked by workers that no
longer heartbeat.

For more detail, see [Worker Lifecycle](workers.md) and
[Architecture](architecture.md).

### Scheduling

Scheduling determines when a job becomes runnable. A job can be available
immediately or scheduled for a later time. Graphile Worker RS also exposes cron
support through `Cron` and `CronBuilder` for recurring work.

Use scheduling when the time a job should run matters more than when it was
created, such as sending a reminder later or running a daily report.

For more detail, see [Scheduling Model](scheduling.md).

### Queues and Concurrency

Concurrency controls how many jobs a worker may process at once. Queues add
coordination around groups of jobs. They are useful when some work must be
serialized or separated from other work.

The database schema includes `_private_job_queues`, which tracks queue names for
serialized job execution. Worker configuration controls the amount of parallel
work a process can take on.

For more detail, see [Queues and Concurrency](queues.md).

### Database Schema

Graphile Worker RS manages its own PostgreSQL schema and migrations. The default
schema name is `graphile_worker`, and the crate documentation identifies these
core tables:

- `_private_jobs` stores job data, state, and execution metadata.
- `_private_tasks` tracks registered task types.
- `_private_job_queues` manages queue names for serialized job execution.
- `_private_workers` tracks active worker instances.

For more detail, see [Database Schema](database-schema.md).

## How The Pieces Fit Together

In a typical application:

1. Define one Rust type per task and implement `TaskHandler`.
2. Configure a `WorkerOptions` value with a PostgreSQL pool and task
   definitions.
3. Initialize the worker so migrations and setup can run.
4. Add jobs from application code or utilities.
5. Run one or more worker processes to execute runnable jobs.

The important boundary is between tasks and jobs. A task is code compiled into
your application. A job is data stored in PostgreSQL that says which task should
run, with which payload, and under which execution rules.

## Where To Go Next

Start with [Architecture](architecture.md) for the system-level view, then read
[Tasks and Payloads](tasks.md) and [Worker Lifecycle](workers.md) to understand
the main programming model. After that, use [Scheduling Model](scheduling.md),
[Queues and Concurrency](queues.md), and [Database Schema](database-schema.md)
for the specific behavior you need to configure or operate.
