# Compatibility

Graphile Worker RS is a Rust implementation of the PostgreSQL-backed job queue
used by Node Graphile Worker. It is designed to use the same database schema and
can run side by side with Node workers when both runtimes understand the schema
revision installed in PostgreSQL.

This page focuses on database and operational compatibility. Rust task handler
code is not portable to Node.js, and Node task functions are not portable to
Rust; the compatibility boundary is the jobs stored in PostgreSQL.

## What Is Shared

Graphile Worker RS manages a PostgreSQL schema, `graphile_worker` by default.
The schema stores queued jobs, task identifiers, queue locks, migration state,
and optional worker recovery state.

The current migrations install the same public-facing objects in the
`graphile_worker` schema that applications typically interact with:

- `add_job(...)` schedules one job.
- `add_jobs(...)` schedules many jobs.
- `remove_job(...)` removes a keyed job.
- `complete_jobs(...)`, `reschedule_jobs(...)`, `permanently_fail_jobs(...)`,
  and `force_unlock_workers(...)` administer jobs.
- `jobs` is a compatibility view over the private job tables.
- `migrations` is the migration ledger.

Internally, newer migrations store queue data in private tables such as
`_private_jobs`, `_private_tasks`, and `_private_job_queues`. Application code
should prefer the public SQL functions and the `jobs` view instead of depending
on the private tables directly.

## Coexisting With Node Workers

Rust and Node workers can process jobs from the same schema when they register
handlers for the same task identifiers and agree on the payload shape.

For example, a job scheduled through SQL can be picked up by any compatible
worker that has a handler for `send_email`:

```sql
select graphile_worker.add_job(
    identifier => 'send_email',
    payload => '{"to":"ada@example.com","subject":"Welcome"}'::json,
    queue_name => 'mailers',
    run_at => now(),
    max_attempts => 25,
    job_key => 'welcome:ada@example.com',
    priority => 0,
    flags => null,
    job_key_mode => 'replace'
);
```

The matching Rust handler must use the same task identifier:

```rust,ignore
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct SendEmail {
    to: String,
    subject: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}
```

The job payload is stored as JSON. Keep payloads language-neutral: use stable
field names, avoid Rust-only serialization assumptions, and make nullable or
optional fields explicit when both Rust and Node workers may consume the same
task.

## Schema Selection

The default schema is `graphile_worker`, but Graphile Worker RS can be pointed at
another schema during worker setup:

```rust,ignore
let worker = graphile_worker::WorkerOptions::default()
    .schema("graphile_worker")
    .define_job::<SendEmail>()
    .pg_pool(pg_pool)
    .init()
    .await?;
```

Use the same schema name when you want Rust and Node workers to share a queue.
Use separate schemas when you want isolation, for example during a staged
migration or when two applications have unrelated task identifiers.

## Important Differences

Graphile Worker RS is mostly compatible with Node Graphile Worker, but it is not
the same process model or API surface.

The README documents one runtime-level difference: Node Graphile Worker gives
each process its own `worker_id`; Graphile Worker RS uses one worker id and
processes jobs within the async runtime according to the configured concurrency.

The Rust crate exposes Rust-specific APIs such as `WorkerOptions`,
`TaskHandler`, `WorkerUtils`, runtime feature selection, typed job definitions,
and optional recovery configuration. Those are Rust conveniences around the
shared database model, not Node APIs.

Graphile Worker RS also adds recovery support in the current schema through
`_private_workers` and SQL functions such as `worker_heartbeat`,
`worker_deregister`, `list_stale_workers`, `list_orphan_locked_workers`,
`recover_dead_worker_jobs`, and `delete_stale_workers`. Recovery is opt-in in
the Rust worker configuration. If you run mixed Rust and Node workers, enable
and test recovery behavior deliberately so that every worker process in the
shared schema is compatible with your operational expectations.

## Migration Behavior

Graphile Worker RS runs ordered migrations and records them in
`graphile_worker.migrations`. The migration set is idempotent: tests install a
fresh schema, add a job, rerun migrations multiple times, and verify that the job
remains present.

The migration code can also take over an existing schema that already has a
Graphile Worker-style `migrations` table. Tests cover a pre-existing migrations
table containing migration `1`, then run the Rust migrations, add a job, and
rerun migrations without losing that job.

There are two compatibility checks to plan for:

- If the database contains a breaking migration id newer than this Rust crate
  knows about, migration aborts instead of downgrading or guessing. Newer
  non-breaking migration ids log a warning and continue.
- Migration 11 refuses to run while recent jobs are locked. Tests set
  `locked_at` and `locked_by` on an existing job and expect the migration to
  fail with a locked-job error.

For production coexistence, treat schema upgrades as a coordinated operation:

1. Check that every Rust and Node worker version you plan to run supports the
   target database schema revision.
2. Drain or stop workers before migrations that rewrite job tables or private
   structures.
3. Confirm there are no active locks before crossing migration 11.
4. Start one runtime first, schedule a simple job, and verify the other runtime
   can see or process jobs with the same task identifier before moving real
   traffic.

## Migration Strategies

For gradual migration from Node to Rust, prefer one of these approaches:

- Shared schema: keep Node workers running, add Rust workers with handlers for a
  small set of task identifiers, and move task ownership incrementally.
- Separate schema: run Rust workers in a different schema while validating new
  task implementations, then move scheduling code to the shared schema when the
  payload and retry behavior are proven.
- SQL scheduling boundary: schedule jobs through `graphile_worker.add_job` or
  `add_jobs` from application code so the producer does not depend on either
  runtime's client API.

When sharing a schema, avoid registering different behavior for the same task
identifier in both runtimes unless the handlers are intentionally equivalent.
PostgreSQL stores the task identifier, not the language runtime that should own
the job.

Related pages:

- [Database schema](../concepts/database-schema.md)
- [Workers](../concepts/workers.md)
- [Migrations](../operations/migrations.md)
- [Job management](../guides/job-management.md)
