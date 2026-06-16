# Local Queue

Local Queue is an optional worker-side cache for high-throughput workloads. When
enabled, the worker fetches a batch of jobs from PostgreSQL, keeps them locked
locally, and lets worker concurrency drain that local batch without another
database round trip for every job.

Use it when many small jobs make database fetch overhead noticeable. For slow or
rare jobs, the default direct database fetch path is usually simpler and keeps
less work locked inside a single process.

## Basic Configuration

Enable the feature with `WorkerOptions::local_queue`:

```rust,ignore
use graphile_worker::{LocalQueueConfig, RefetchDelayConfig, WorkerOptions};
use std::time::Duration;

let worker = WorkerOptions::default()
    .concurrency(4)
    .local_queue(
        LocalQueueConfig::default()
            .with_size(100)
            .with_ttl(Duration::from_secs(300))
            .with_refetch_delay(
                RefetchDelayConfig::default()
                    .with_duration(Duration::from_millis(100))
                    .with_threshold(10)
                    .with_max_abort_threshold(500),
            ),
    )
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

The same settings are available through the generated builders:

```rust,ignore
use graphile_worker::{LocalQueueConfig, RefetchDelayConfig};
use std::time::Duration;

let config = LocalQueueConfig::builder()
    .size(100)
    .ttl(Duration::from_secs(300))
    .refetch_delay(
        RefetchDelayConfig::builder()
            .duration(Duration::from_millis(100))
            .threshold(10)
            .max_abort_threshold(500)
            .build(),
    )
    .build();
```

## Config Fields

`size` controls the maximum number of jobs fetched and held by each local queue.
The default is `100`. It must be greater than zero and must not exceed
`i32::MAX`.

`ttl` controls how long fetched jobs may remain unclaimed in the local queue
before they are returned to PostgreSQL. The default is five minutes.

`refetch_delay` is optional. It slows the next fetch after a low-yield fetch so
the worker does not immediately poll the database again when the queue appears
empty or nearly empty.

`queue_count` controls how many independent local queues run inside one worker.
The default is `1`. It must be greater than zero. `size` applies per queue, so
the maximum local capacity is `size * queue_count`.

If `queue_count` is greater than worker concurrency, it is capped at the
concurrency value because each local queue needs a worker draining it.

## Modes

Each local queue moves through a small state machine:

- `Polling`: the queue may fetch jobs from PostgreSQL.
- `Waiting`: jobs are cached locally and are being served from memory.
- `TtlExpired`: the local cache TTL expired and unclaimed jobs are being
  returned to PostgreSQL.
- `Released`: the local queue has shut down and will not provide more jobs.

When the cached batch is drained, the queue returns to `Polling`. When TTL
expires, unclaimed jobs are returned to PostgreSQL and later fetches can poll
again.

## Refetch Delay

Refetch delay starts when a fetch returns fewer jobs than requested and the
number of jobs fetched is not greater than the configured threshold.

```rust,ignore
use graphile_worker::{LocalQueueConfig, RefetchDelayConfig};
use std::time::Duration;

let config = LocalQueueConfig::default().with_refetch_delay(
    RefetchDelayConfig::default()
        .with_duration(Duration::from_millis(200))
        .with_threshold(5),
);
```

The actual delay is jittered between half of `duration` and the full `duration`.
`duration` must not be larger than the worker `poll_interval`.

PostgreSQL notifications can wake the local queue sooner. While a refetch delay
is active, incoming job pulses increment an internal counter. When that counter
reaches the abort threshold, the delay is aborted and the queue fetches again.
If `max_abort_threshold` is not set, the abort threshold is randomized up to
`5 * size`. If it is set, the randomized abort threshold is capped by that
value.

## Distribution

Local Queue does not change job ownership rules. Jobs are still locked in
PostgreSQL under the worker id, and the worker's normal concurrency controls how
many handlers can run at the same time.

With a single local queue, one batch is shared by the worker tasks. With
`queue_count` greater than one, the worker starts multiple independent local
queues and checks them round-robin. This can improve throughput for very small,
high-volume jobs, but it can also lock more work inside one process:

```rust,ignore
use graphile_worker::LocalQueueConfig;

let config = LocalQueueConfig::default()
    .with_size(50)
    .with_queue_count(4);
```

In this example, up to `200` jobs may be locked locally if worker concurrency is
at least `4`.

## Shutdown And TTL

Local Queue returns unclaimed cached jobs to PostgreSQL in two cases:

- the queue TTL expires while jobs are still cached;
- the worker shuts down and releases the local queue.

Returning jobs is retried internally. Jobs that are already running are handled
by the worker's normal shutdown behavior; the local queue release path is for
jobs that were fetched into the local cache but not yet claimed by a handler.

Choose `ttl` based on how much work you are comfortable locking inside one
process if handlers are slower than the local batch drain rate.

## Forbidden Flags

Workers configured with forbidden flags do not use the local queue cache.
During worker initialization, a non-empty forbidden flag list disables the local
queue configuration for that worker.

This preserves flag filtering semantics: jobs with forbidden flags are skipped
by the worker, and eligible jobs are fetched directly from PostgreSQL with the
forbidden flag filter applied.
