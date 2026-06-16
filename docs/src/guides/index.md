# Guides

Use these guides when you already know the basic worker shape and want to wire
Graphile Worker RS into a real application flow. If you are new to the crate,
start with [Quick Start](../getting-started/quick-start.md) or
[First Worker](../getting-started/first-worker.md), then come back here for
focused workflows.

## Pick a Guide

| Goal | Start here |
| --- | --- |
| Add work to the queue from application code | [Scheduling Jobs](scheduling-jobs.md) |
| Group related work and control batch behavior | [Batch Jobs](batch-jobs.md) |
| Run recurring work on a cron schedule | [Cron Jobs](cron.md) |
| Understand low-latency in-process job dispatch | [Local Queue](local-queue.md) |
| Recover jobs after crashed or stale workers | [Worker Recovery](recovery.md) |
| Observe or customize job lifecycle events | [Lifecycle Hooks](hooks.md) |
| Inspect, retry, or manage jobs from code | [Job Management](job-management.md) |

## Common Starting Points

### Define a Task Handler

Most guides assume you have a task type that implements `TaskHandler`. A task
has a stable identifier, a serializable payload, and an async `run` method:

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
    body: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending email to {}", self.to);
        Ok::<(), String>(())
    }
}
```

For the full setup flow, see [First Worker](../getting-started/first-worker.md).

### Register Jobs on a Worker

Register task handlers with `WorkerOptions`, then provide a PostgreSQL pool and
initialize the worker:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(5)
    .schema("graphile_worker")
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

For configuration details, see
[Worker Options](../configuration/worker-options.md),
[Runtime, TLS, and Drivers](../configuration/runtime-drivers.md), and
[Application State and Extensions](../configuration/app-state.md).

### Choose the Operational Path

After the worker is running, the guide you need depends on how jobs should be
created and supervised:

- Use [Scheduling Jobs](scheduling-jobs.md) for ordinary background work such
  as sending emails, generating PDFs, or deferring slow application tasks.
- Use [Cron Jobs](cron.md) when the work should be created from a recurring
  schedule instead of a user request.
- Use [Worker Recovery](recovery.md) for deployments where a process crash,
  network partition, forced abort, or orchestrator shutdown could leave jobs
  locked.
- Use [Lifecycle Hooks](hooks.md) when you need code to run around job events.
- Use [Job Management](job-management.md) when your application needs utility
  operations for existing jobs.

## Related References

- [Tasks and Payloads](../concepts/tasks.md) explains the task model used by
  these guides.
- [Queues and Concurrency](../concepts/queues.md) covers how workers limit and
  coordinate job execution.
- [Shutdown](../configuration/shutdown.md) describes graceful shutdown behavior.
- [Migrations](../operations/migrations.md) covers preparing the PostgreSQL
  schema used by the worker.
- [Troubleshooting](../reference/troubleshooting.md) collects common diagnosis
  paths when a guide does not match what you see at runtime.
