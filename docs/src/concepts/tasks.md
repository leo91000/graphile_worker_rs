# Tasks and Payloads

Tasks are the typed boundary between queued JSON jobs and your Rust code. A task
is a Rust payload type that implements `TaskHandler`; Graphile Worker RS uses the
task identifier stored with the job to select the handler, deserializes the job
payload into that type, and then calls `run`.

## TaskHandler

A normal task handler needs four pieces:

- a payload type that implements `Serialize` and `Deserialize`
- a globally unique `IDENTIFIER`
- an async `run` method
- a registered job definition on the worker

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
        println!("Sending email to {} with subject: {}", self.to, self.subject);
        Ok::<(), String>(())
    }
}
```

`run` receives `self` as the deserialized payload. Returning `()` marks the job as
complete. Returning `Result<(), E>` marks `Ok(())` as complete and `Err(error)` as
failed, using the error's debug representation.

Register the handler before running the worker:

```rust,ignore
let worker = WorkerOptions::default()
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

For reusable modules, expose definitions instead of making callers know every
type:

```rust,ignore
use graphile_worker::{JobDefinition, TaskHandler};

pub fn jobs() -> [JobDefinition; 2] {
    [SendEmail::definition(), SendDailyReport::definition()]
}
```

## Identifiers

`IDENTIFIER` is the string stored with the job. It must be unique across all task
types registered by your application, because the worker uses it to match queued
jobs to handlers.

Prefer stable, descriptive, lowercase names:

```rust,ignore
impl TaskHandler for ProcessPayment {
    const IDENTIFIER: &'static str = "process_payment";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}
```

When adding typed jobs, the identifier comes from the task type:

```rust,ignore
helpers
    .add_job(
        SendEmail {
            to: "alice@example.com".into(),
            subject: "Welcome!".into(),
        },
        JobSpecBuilder::new().build(),
    )
    .await?;
```

When adding raw jobs, you provide the identifier and JSON payload yourself. This
is useful for heterogeneous batches, but it skips the compile-time connection
between payload type and handler:

```rust,ignore
utils
    .add_raw_jobs(&[
        RawJobSpec {
            identifier: "send_email".into(),
            payload: serde_json::json!({
                "to": "dave@example.com",
                "subject": "Notification"
            }),
            spec: JobSpec::default(),
        },
        RawJobSpec {
            identifier: "process_payment".into(),
            payload: serde_json::json!({ "user_id": 123, "amount": 50 }),
            spec: JobSpec::default(),
        },
    ])
    .await?;
```

## Payload Serialization

Task payloads are serialized to JSON when jobs are added and deserialized from
JSON when jobs run. The implementing type is the payload shape:

```rust,ignore
#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}
```

If deserialization fails, the task is treated as failed and the deserialization
error is reported as the failure reason. Keep payload structs explicit and avoid
depending on data that is not in the job payload unless it comes from
`WorkerContext`.

Batch insertion can still be type-safe when every job has the same task type:

```rust,ignore
let spec = JobSpec::default();
let emails = vec![
    (
        SendEmail {
            to: "alice@example.com".into(),
            subject: "Welcome!".into(),
        },
        &spec,
    ),
    (
        SendEmail {
            to: "bob@example.com".into(),
            subject: "Welcome!".into(),
        },
        &spec,
    ),
];

utils.add_jobs::<SendEmail>(&emails).await?;
```

Use job specs for scheduling, priority, and other queue behavior. See
[Scheduling](./scheduling.md) and [Queues](./queues.md) for the surrounding job
controls.

## Batch Handlers

`BatchTaskHandler` is for one database job whose payload is a JSON array of item
payloads. `Self` is the item type, not `Vec<Self>`.

```rust,ignore
use graphile_worker::{
    BatchTaskHandler, IntoBatchTaskHandlerResult, JobSpecBuilder, WorkerContext,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct PendingNotification {
    user_id: String,
    message_id: String,
}

impl BatchTaskHandler for PendingNotification {
    const IDENTIFIER: &'static str = "send_notifications";

    async fn run_batch(
        items: Vec<Self>,
        _ctx: WorkerContext,
    ) -> impl IntoBatchTaskHandlerResult {
        let mut results = Vec::with_capacity(items.len());

        for item in items {
            results.push(
                send_notification(item)
                    .await
                    .map_err(|error| error.to_string()),
            );
        }

        results
    }
}
```

Register batch handlers with `define_batch_job` and add batch jobs with
`add_batch_job`:

```rust,ignore
let worker = WorkerOptions::default()
    .define_batch_job::<PendingNotification>()
    .pg_pool(pg_pool)
    .init()
    .await?;

worker
    .create_utils()
    .add_batch_job(
        vec![
            PendingNotification {
                user_id: "1".into(),
                message_id: "a".into(),
            },
            PendingNotification {
                user_id: "1".into(),
                message_id: "b".into(),
            },
        ],
        JobSpecBuilder::new().build(),
    )
    .await?;
```

A batch handler can return:

- `()` for complete success
- `Result<(), E>` to complete or fail the whole batch
- `Vec<Result<(), E>>` for per-item results
- `BatchTaskResult<E>` directly

For per-item results, the vector must have the same length and order as the
input items. Failed positions are retried with their original payload values;
successful positions are removed from the retry payload. If the stored payload is
not a JSON array, or the result count does not match the item count, the batch
job fails.

## WorkerContext

Both `TaskHandler::run` and `BatchTaskHandler::run_batch` receive a
`WorkerContext`. Use the payload fields on `self` for job input, and use the
context for worker-provided data such as job metadata and registered extensions.

```rust,ignore
#[derive(Clone, Debug)]
struct AppState {
    api_key: String,
}

impl TaskHandler for ProcessUserTask {
    const IDENTIFIER: &'static str = "process_user";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let app_state = ctx.get_ext::<AppState>().unwrap();
        call_service(&app_state.api_key, &self.user_id).await?;
        Ok::<(), String>(())
    }
}
```

Keep durable job input in the payload. Keep process-local resources, clients,
counters, and configuration in extensions reached through `WorkerContext`.

## Versioning Payloads

Queued jobs may outlive the code version that created them. Treat each payload as
a stored API contract:

- keep identifiers stable unless you intentionally migrate producers and workers
- add optional fields when older queued jobs may not contain new data
- prefer explicit field names over positional or loosely shaped JSON
- avoid removing or renaming required fields while old jobs can still run
- use a new identifier when the meaning of a task changes incompatibly

These rules matter for normal jobs and batch jobs. Batch jobs store an array of
payload items, so each item shape needs the same compatibility care as a normal
task payload.
