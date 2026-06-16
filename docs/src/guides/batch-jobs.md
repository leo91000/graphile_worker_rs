# Batch Jobs

Graphile Worker RS has two batch-related APIs:

- `add_jobs` and `add_raw_jobs` insert many ordinary jobs in one call.
- `BatchTaskHandler` processes a JSON array stored in one job payload.

Use bulk adds when you want to enqueue many independent jobs efficiently. Use a
batch handler when one task run should receive a group of items and decide which
items completed or failed.

## Bulk Add Ordinary Jobs

`WorkerUtils::add_jobs` inserts multiple jobs for the same typed task. Each item
is still stored as its own job row and is processed by the normal `TaskHandler`
implementation.

```rust,ignore
use graphile_worker::{JobSpec, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> Result<(), String> {
        send_email(self.to, self.subject).await
    }
}

let spec = JobSpec::default();
let jobs = [
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

let added_jobs = worker.create_utils().add_jobs::<SendEmail>(&jobs).await?;
```

The `JobSpec` is supplied per job. You can reuse one spec for all jobs or pass
different specs to set different priorities, queues, run times, or other job
options.

```rust,ignore
use graphile_worker::{JobSpec, JobSpecBuilder};

let urgent = JobSpecBuilder::new().priority(-10).build();
let normal = JobSpec::default();

worker
    .create_utils()
    .add_jobs::<SendEmail>(&[
        (urgent_email, &urgent),
        (normal_email, &normal),
    ])
    .await?;
```

For heterogeneous batches, use `add_raw_jobs`. Each `RawJobSpec` carries its own
task identifier, JSON payload, and `JobSpec`.

```rust,ignore
use graphile_worker::{JobSpec, JobSpecBuilder, RawJobSpec};
use serde_json::json;

let added_jobs = worker
    .create_utils()
    .add_raw_jobs(&[
        RawJobSpec {
            identifier: "send_email".into(),
            payload: json!({
                "to": "dave@example.com",
                "subject": "Notification"
            }),
            spec: JobSpec::default(),
        },
        RawJobSpec {
            identifier: "process_payment".into(),
            payload: json!({ "user_id": 123, "amount": 50 }),
            spec: JobSpecBuilder::new().priority(-10).build(),
        },
    ])
    .await?;
```

Bulk-added ordinary jobs are fetched and run like any other job. They do not
call `run_batch`.

## Batch Handlers

A batch job is one job row whose payload is a JSON array. Register it with
`define_batch_job` and implement `BatchTaskHandler` for the item type stored in
that array.

```rust,ignore
use graphile_worker::{
    BatchTaskHandler, IntoBatchTaskHandlerResult, JobSpec, WorkerContext,
    WorkerOptions,
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

let worker = WorkerOptions::default()
    .define_batch_job::<PendingNotification>()
    // ... database pool and other options
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
        JobSpec::default(),
    )
    .await?;
```

The stored payload for that job is an array:

```json
[
  { "user_id": "1", "message_id": "a" },
  { "user_id": "1", "message_id": "b" }
]
```

`add_batch_job` requires at least one item. If the final payload is not a JSON
array, or is an empty array, the job is rejected before insertion.

## Return Values and Retries

Batch handlers can report success for the whole batch or per item.

Returning `Ok(())`, `()`, or `BatchTaskResult::Complete` completes the whole job.

```rust,ignore
impl BatchTaskHandler for PendingNotification {
    const IDENTIFIER: &'static str = "send_notifications";

    async fn run_batch(
        _items: Vec<Self>,
        _ctx: WorkerContext,
    ) -> impl IntoBatchTaskHandlerResult {
        Ok::<(), String>(())
    }
}
```

Returning one result per payload item lets Graphile Worker RS retry only the
failed items. The result vector must have the same length as the input `items`.

```rust,ignore
impl BatchTaskHandler for PendingNotification {
    const IDENTIFIER: &'static str = "send_notifications";

    async fn run_batch(
        items: Vec<Self>,
        _ctx: WorkerContext,
    ) -> impl IntoBatchTaskHandlerResult {
        items
            .into_iter()
            .map(|item| {
                if should_retry(&item) {
                    Err(format!("failed {}", item.message_id))
                } else {
                    Ok(())
                }
            })
            .collect::<Vec<_>>()
    }
}
```

If only some item results fail, the worker removes successful items from the
payload and leaves a retry job containing only the failed items. If the whole
handler fails, or if the result vector length does not match the number of input
items, the original payload is retried.

Invalid stored payloads are treated as job failures:

- A batch handler requires the job payload to be a JSON array.
- Every array item must deserialize into the batch item type.
- Deserialization errors leave the original payload on the retried job.

## Adding Batch Jobs From a Handler

`WorkerContext` can enqueue a typed batch job from inside another task.

```rust,ignore
impl TaskHandler for ParentJob {
    const IDENTIFIER: &'static str = "batch_parent_job";

    async fn run(self, ctx: WorkerContext) -> Result<(), String> {
        ctx.add_batch_job(
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
            JobSpec::default(),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
    }
}
```

The inserted job uses the batch task identifier and stores the provided items as
one JSON array payload.

## Internal Completion and Failure Batching

The worker also batches some internal persistence work when completing or
failing jobs. Completion and failure requests are collected for a configured
delay, then flushed together. On shutdown, the batcher drains queued requests and
flushes them before exiting. If the batcher has already closed, the worker falls
back to direct completion or failure persistence for that job.

This internal persistence batching is separate from `BatchTaskHandler`: it does
not change how many payload items your handler receives.
