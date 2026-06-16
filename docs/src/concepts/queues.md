# Queues and Concurrency

Graphile Worker RS uses PostgreSQL as the source of truth for runnable jobs, and
worker concurrency decides how many jobs a worker may execute at the same time.
Queues add another layer of control: they let you group jobs by workload and
serialize jobs that share the same queue name.

Use queues when one class of work should not interfere with another, or when a
sequence of jobs must not run in parallel.

## Assigning Jobs to Queues

Jobs can be assigned to a queue with `JobSpec::queue_name`. The field is
optional; when it is not set, the job is added without an explicit queue name.

```rust,ignore
use graphile_worker::JobSpecBuilder;

worker
    .create_utils()
    .add_job(
        SendEmail {
            to: "user@example.com".to_string(),
        },
        JobSpecBuilder::new()
            .queue_name("mail")
            .build(),
    )
    .await?;
```

The same option is available when building `JobSpec` directly:

```rust,ignore
use graphile_worker::JobSpec;

let spec = JobSpec {
    queue_name: Some("exports".to_string()),
    ..Default::default()
};
```

Queue names are data, not task identifiers. Different task handlers can share a
queue when they must be serialized together, and the same task handler can use
different queues for different tenants, accounts, or workload classes.

## Serial Queues

A queue is useful when jobs for the same resource must run one at a time. For
example, jobs that update the same external account can all use a queue derived
from that account id:

```rust,ignore
let queue_name = format!("account:{}", account_id);

worker
    .create_utils()
    .add_job(
        SyncAccount { account_id },
        JobSpecBuilder::new()
            .queue_name(queue_name)
            .build(),
    )
    .await?;
```

While a queued job is in progress, Graphile Worker RS records the queue lock in
the database. Tests assert that a named queue has a `locked_at` timestamp,
`locked_by` worker id, and a job count while its job is running. When the job
finishes, the completed job is removed and the queue can be used by later work.

This makes queue names a practical serialization primitive:

- Use one queue name for all jobs that must run in order.
- Use distinct queue names for jobs that may run at the same time.
- Keep queue names stable and deterministic when they represent a shared
  resource.

## Worker Concurrency

Worker concurrency controls how many jobs a worker can execute at once.

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(5)
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

With higher concurrency, independent queues can run in parallel. The
concurrency tests enqueue five jobs into five different queues and configure
the worker with `concurrency(10)`; all five jobs are picked up and left
in progress at the same time.

Without increasing concurrency, work is effectively drained one job at a time in
the tested `run_once` path. That is useful for small deployments or jobs that
should not overlap, but it means one slow job can delay unrelated work.

## Workload Isolation

Queues and concurrency solve different parts of workload isolation.

Use queue names to protect resources:

```rust,ignore
let spec = JobSpecBuilder::new()
    .queue_name(format!("project:{}", project_id))
    .build();
```

Use worker concurrency to decide how many independent jobs the process can run:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .concurrency(8)
    .define_job::<RenderPreview>()
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Common patterns:

- Per-tenant queues: `tenant:42`
- Per-project queues: `project:abc123`
- Per-external-resource queues: `stripe:acct_123`
- Shared workload queues: `mail`, `exports`, `webhooks`

Choose the narrowest queue name that protects the resource you care about. A
single global queue is simple, but it serializes everything behind the slowest
job in that queue. Very fine-grained queue names allow more parallelism, but
they only protect resources that are named consistently.

## Local Queue

The local queue is an in-worker cache of jobs fetched from PostgreSQL. It exists
to reduce polling and provide low-latency processing while the database remains
the durable source of truth.

```rust,ignore
use graphile_worker::LocalQueueConfig;

let worker = graphile_worker::WorkerOptions::default()
    .concurrency(3)
    .local_queue(LocalQueueConfig::builder().size(10).build())
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

`LocalQueueConfig` includes:

- `size`: maximum number of jobs each local queue may fetch and hold at once.
  The default is `100`.
- `ttl`: how long locally fetched jobs may stay unclaimed before being returned
  to the database. The default is five minutes.
- `refetch_delay`: an optional delay strategy used when a fetch returns fewer
  jobs than requested.
- `queue_count`: number of independent local queues to run inside this worker.
  The default is `1`.

When `queue_count` is greater than one, `size` applies to each local queue. For
example, `size = 3` and `queue_count = 4` allow up to twelve jobs to be locked
locally across the worker. Tests also assert that `queue_count` is capped by
worker concurrency, because each local queue needs at least one worker draining
it.

```rust,ignore
let local_queue = LocalQueueConfig::default()
    .with_size(3)
    .with_queue_count(4);

let worker = graphile_worker::WorkerOptions::default()
    .concurrency(5)
    .local_queue(local_queue)
    .define_job::<SmallFastJob>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Multiple local queues can improve throughput for very small, high-volume jobs
by allowing several fetch batches in parallel. They can also lock more jobs
inside one worker, so keep `size` lower when increasing `queue_count` and
benchmark with realistic workloads.

## Practical Guidance

Start with worker concurrency and explicit queue names before tuning local queue
internals.

- Increase `concurrency` when unrelated jobs should run in parallel and the
  database pool, runtime, and downstream services can handle the extra work.
- Add `queue_name` when jobs for the same tenant, project, account, or external
  system must not overlap.
- Use separate queue names for unrelated workloads so slow serial work does not
  block everything else.
- Enable and tune `local_queue` when polling overhead or very small jobs become
  a bottleneck.
- Raise `queue_count` only after measuring; it increases parallel fetch capacity
  and the number of jobs a worker can hold locally.
