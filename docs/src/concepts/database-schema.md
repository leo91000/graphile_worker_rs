# Database Schema

Graphile Worker RS stores jobs and coordination state in PostgreSQL. By
default it uses the `graphile_worker` schema, and migrations create the tables,
types, indexes, functions, and views that the worker runtime needs.

Treat this schema as owned by Graphile Worker RS. Application code should add
jobs through the Rust API or the installed SQL functions instead of writing
directly to private tables.

## Schema Name

The default schema name is `graphile_worker`. Migration SQL is written with a
schema placeholder and executed against the configured schema, so installations
can use another schema name when the worker is configured that way.

For example, the migration executor replaces the schema placeholder before it
runs each SQL statement:

```sql
:GRAPHILE_WORKER_SCHEMA
```

Most applications can leave the default schema in place.

## Installed Objects

Current migrations install and maintain these main storage tables:

| Object | Purpose |
| --- | --- |
| `_private_jobs` | Stores queued jobs, scheduling fields, attempts, locks, flags, keys, and errors. |
| `_private_tasks` | Stores task identifiers and maps them to internal task ids. |
| `_private_job_queues` | Stores named queues and queue lock state for serialized queue execution. |
| `_private_workers` | Stores worker heartbeat metadata used by stale-worker recovery. |
| `migrations` | Records applied migration ids and whether each migration is breaking. |

The worker also installs SQL functions used by the Rust implementation,
including job insertion, completion, failure/rescheduling operations, worker
heartbeat, stale-worker listing, and dead-worker recovery functions.

The exact private table layout can change across migrations. Prefer the
documented APIs and the installed functions for integration points.

## Public Jobs View

The migrations expose a `jobs` view over the private tables. It joins jobs to
their task identifier and queue name so operators can inspect queue state
without depending on every private table detail.

```sql
SELECT
    id,
    queue_name,
    task_identifier,
    priority,
    run_at,
    attempts,
    max_attempts,
    last_error,
    created_at,
    updated_at,
    key,
    locked_at,
    locked_by,
    revision,
    flags
FROM graphile_worker.jobs
ORDER BY priority, run_at, id;
```

Use this for diagnostics and operational visibility. Avoid updating through the
view or relying on it as an application-owned table.

## Jobs

A job row tracks the work to perform and the state needed to schedule it:

- the task identifier, stored through `_private_tasks`;
- the JSON payload;
- optional queue membership, stored through `_private_job_queues`;
- `run_at`, `priority`, `attempts`, and `max_attempts`;
- optional `key` and `flags`;
- lock fields, error text, revision, and timestamps.

Adding jobs through the worker APIs keeps these related records consistent.
The SQL migrations also install `add_job` and `add_jobs` functions, which create
task and queue rows as needed and notify workers that jobs were inserted.

```rust,ignore
use graphile_worker::{JobSpec, WorkerUtils};
use serde_json::json;

let worker_utils = WorkerUtils::new(database, "graphile_worker");

worker_utils
    .add_raw_job(
        "send_email",
        json!({ "user_id": 42 }),
        JobSpec::default(),
    )
    .await?;
```

## Queues

Jobs may belong to a named queue. Queue rows carry lock state so the worker can
serialize jobs within the same named queue while still allowing unrelated queues
or unqueued jobs to make progress.

Queue names are an internal scheduling primitive, not a separate application
model. Let `add_job` or `add_jobs` create queue rows as needed.

## Workers

Workers register heartbeat state in `_private_workers`. The installed functions
include:

- `worker_heartbeat`, which inserts or updates a worker heartbeat and optional
  metadata;
- `worker_deregister`, which removes a worker row;
- `list_stale_workers` and `list_orphan_locked_workers`, which identify workers
  or locks that need recovery;
- `recover_dead_worker_jobs`, which clears locks for selected workers and
  reschedules affected jobs;
- `delete_stale_workers`, which removes stale worker rows.

These functions are part of the worker recovery machinery. Application code
normally configures and runs workers rather than calling them directly.

## Migrations

Graphile Worker RS ships its migration SQL with the crate and records applied
migrations in the schema's `migrations` table. The current migration registry
contains 20 migrations.

Migrations are designed to be repeatable after they have been applied: running
migration again leaves existing jobs in place. The migration tests also cover
taking over from a pre-existing `migrations` table.

Some migrations are marked as breaking in the migration metadata. The current
breaking migration ids are:

```text
1, 3, 11, 13, 14, 16
```

Migration 11 is intentionally cautious around locked jobs. If jobs are locked
when that migration runs, migration fails with a dedicated locked-job error
instead of silently changing active job state.

## Compatibility

The worker checks migration revision compatibility after applying migrations.
If the database contains a breaking migration id newer than the worker knows
about, migration aborts instead of letting an older binary run against a future
schema. If the newer recorded migration is not marked as breaking, the worker
logs a compatibility warning and continues.

That check protects private schema compatibility. It also means deployment
order matters when multiple services share the same worker schema: avoid rolling
back to a binary that cannot understand the breaking schema version already
installed in the database.

For operational SQL, keep queries conservative:

```sql
-- Good for inspection
SELECT id, task_identifier, run_at, attempts, max_attempts, last_error
FROM graphile_worker.jobs
WHERE attempts >= max_attempts
ORDER BY updated_at DESC;
```

Avoid SQL that inserts, updates, deletes, or assumes private column names in
`_private_*` tables unless you are working on Graphile Worker RS itself.
