# Architecture

Graphile Worker RS is a PostgreSQL-backed job queue. Application code writes
jobs into the Graphile Worker schema, worker processes lock ready jobs with
PostgreSQL row locks, task handlers run in the configured async runtime, and
the worker persists completion, retry, failure, or recovery state back to the
database.

The public crate is split into a small set of user-facing concepts:

- `WorkerOptions` configures a worker, registers task handlers, and creates a
  `Worker`.
- `Worker` runs the job loop until shutdown.
- `WorkerUtils` adds and manages jobs outside the worker loop.
- `TaskHandler` defines the Rust code that handles a job payload.
- `JobSpec` configures scheduling, queues, attempts, priority, flags, and job
  keys for inserted jobs.

Internally, the root crate coordinates smaller workspace crates. The query
crate owns SQL construction, the job/task/spec crates define shared data
structures, the lifecycle hook crate exposes extension points, and the runtime
facade keeps the worker logic independent of a specific async runtime feature.

## Data Flow

The normal path from enqueue to release is:

1. Application code adds a job through `WorkerUtils` or the lower-level
   `add_job` SQL wrapper.
2. PostgreSQL stores the job in the worker schema and emits the database
   notification used by listeners.
3. A running worker receives a signal from `LISTEN jobs:insert`, periodic
   polling, `run_once`, or an internal local-queue signal.
4. Worker tasks fetch ready jobs with `for update skip locked`, filtered to the
   task identifiers registered in this worker.
5. The worker builds a `WorkerContext`, runs the matching `TaskHandler`, and
   catches task errors and panics.
6. The release path deletes completed jobs, reschedules retryable failures, or
   returns interrupted jobs for recovery.

```text
application
  |
  | add_job(identifier, payload, JobSpec)
  v
PostgreSQL worker schema
  |
  | LISTEN/NOTIFY or polling
  v
Worker job loop
  |
  | get_job / batch_get_jobs
  v
TaskHandler::run(WorkerContext)
  |
  +-- success ----------> complete_job: delete job, unlock queue
  |
  +-- task failure -----> fail_job: save error, unlock, retry later
  |
  +-- shutdown abort ---> recovery return: decrement attempt, unlock, delay
```

## Adding Jobs

The query layer inserts jobs by calling the schema's `add_job` database
function with the task identifier, JSON payload, and `JobSpec` fields:

```rust,ignore
let job = worker
    .create_utils()
    .add_raw_job(
        "send_email",
        serde_json::json!({ "to": "a@example.com" }),
        graphile_worker::JobSpec::default(),
    )
    .await?;
```

The SQL wrapper passes these options to PostgreSQL:

- `identifier`: the task name registered by a handler.
- `payload`: the JSON payload stored with the job.
- `queue_name`: optional queue serialization key.
- `run_at`: optional scheduled time.
- `max_attempts`: optional retry limit.
- `job_key` and `job_key_mode`: optional job de-duplication/update behavior.
- `priority`: lower values are fetched first.
- `flags`: labels that workers may skip through `forbidden_flags`.

When tracing context is active, the insert path can add trace information into
the payload before writing it. If `use_local_time` is enabled and no `run_at`
is supplied, the Rust process time is used as the job's run time; otherwise the
SQL layer uses database time.

## Fetching Jobs

Workers do not scan every job type. During initialization they know the task
identifiers they can run, and fetch only jobs whose `task_id` is in that set.
The fetch query requires:

- `is_available = true`
- `run_at <= now()`, or the supplied local timestamp
- no forbidden flags selected by this worker
- an available queue when the job belongs to a named queue

Jobs are ordered by `priority asc, run_at asc`, then locked with:

```sql
for update
skip locked
```

After selecting a row, the same query increments `attempts`, sets `locked_by`
to the worker id, and records `locked_at`. For queued jobs, the related
`job_queues` row is also locked by the worker. This is the core coordination
mechanism that lets multiple worker processes share the same PostgreSQL queue
without taking the same job.

## Worker Loop

`Worker::run()` performs the runtime lifecycle in this order:

1. Register the worker when recovery support is enabled.
2. Emit worker start hooks.
3. Spawn recovery background tasks.
4. Run the crontab scheduler and job runner together.
5. Wait for completion and failure batchers to flush on shutdown.
6. Stop recovery tasks, emit shutdown hooks, and deregister the worker.

The job runner listens for job signals. A signal can come from the PostgreSQL
listener, the polling interval, `run_once`, or the local queue. Each signal is
fanned out to the configured concurrency so idle worker tasks can attempt to
fetch jobs.

There are two fetch modes:

- Direct mode fetches one job per worker task with `get_job`.
- Local queue mode prefetches jobs with `batch_get_jobs` and wakes worker
  tasks through an internal signal when cached jobs are available.

Both modes end in the same execution and release logic.

## Running a Task

After a job is fetched, the worker looks up the registered handler by the job's
task identifier. If no identifier or function is found, the job is released
through the failure path.

For a valid job, the worker creates a `WorkerContext` containing the shared
job, database handle, schema, worker id, extensions, task details, and time
mode. It then runs the handler future:

```rust,ignore
impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        // use ctx and the deserialized payload here
        Ok::<(), String>(())
    }
}
```

The execution path records task duration for successful jobs, catches panics,
and watches the worker shutdown signal. If shutdown is requested, the worker
waits for the configured grace period. A task that has not finished by then is
treated as aborted and goes through the recovery release path.

## Completion and Failure

Successful jobs are completed by deleting the job row. If the job belongs to a
queue, completion also clears `locked_by` and `locked_at` on the queue row, but
only when that queue is locked by the same worker id.

Failed jobs are updated instead of deleted:

- `last_error` is set to the persisted error message.
- `run_at` is moved forward by exponential backoff based on `attempts`.
- `locked_by` and `locked_at` are cleared.
- a replacement payload may be written when the handler returned one.
- queued jobs also unlock their queue row.

The worker decides whether a failure will retry by comparing `attempts` with
`max_attempts`. Retryable failures emit job-fail hooks; exhausted jobs emit
permanent-failure hooks. The SQL update is the same shape: a permanently failed
job remains unlocked with its last error recorded and is no longer available
once its attempt limit is reached by the schema availability rules.

Completion and ordinary failures can be batched. The completion and failure
batchers receive release requests through bounded channels, flush them after
the configured delay, and emit hooks only for rows that were actually persisted.
If a batcher is already closed, the worker falls back to direct persistence for
that job.

## Shutdown and Recovery

Shutdown aborts are intentionally not normal task failures. When a running job
is interrupted by the shutdown grace-period timeout, the worker applies job
recovery instead of failure backoff:

- `attempts` is decremented with a floor of zero.
- `locked_by` and `locked_at` are cleared.
- `run_at` is delayed by `interrupted_job_retry_delay` when configured.
- queued jobs also unlock their queue row.
- interruption hooks are emitted when recovery handled the job.

Automatic worker recovery is separate and opt-in. When enabled, workers record
heartbeats, and the recovery sweeper finds stale worker ids or orphaned locks.
Recovered jobs are returned to the queue through the same recovery semantics:
the attempt consumed by fetching the job is reversed, locks are cleared, and a
recovery delay can be applied before the job is visible again.

The recovery SQL layer can also fetch locked jobs for a set of worker ids and
call the schema's `recover_dead_worker_jobs` function to recover jobs owned by
dead workers.

## Workspace Layout

The workspace keeps behavior separated by responsibility:

```text
src/
  lib.rs                 crate exports and top-level module structure
  runner/                worker lifecycle, job loop, execution, release
  batcher/               completion and failure batching
  streams/               job signal and job fetch streams

crates/
  graphile-worker-queries/
    src/add_job/         SQL wrappers for inserting jobs
    src/get_job.rs       single-job locking fetch
    src/batch_get_jobs.rs
                         batched locking fetch for LocalQueue
    src/complete_job.rs  completion persistence
    src/fail_job/        failure persistence
    src/return_jobs/     recovery and interrupted-job return
    src/recover_workers.rs
                         stale-worker recovery queries
```

This layout keeps most PostgreSQL statements in
`graphile-worker-queries`, while `src/runner` owns the orchestration decisions:
when to fetch, how to execute handlers, which release path to use, and when to
flush background components.
