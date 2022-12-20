# Archimedes

**NOT PRODUCTION READY**

Rewrite of [Graphile Worker](https://github.com/graphile/worker) in Rust. If you like this library go sponsor [Benjie](https://github.com/benjie) project, all research has been done by him, this library is only a rewrite in Rust 🦀.
The port should mostly be compatible with `graphile-worker` (meaning you can run it side by side with Node.JS).

The following differs from `Graphile Worker` :
- No support for batch job
- In `Graphile Worker`, each process has it's worker_id. In rust there is only one worker_id, then jobs are processed in your async runtime thread.

Job queue for PostgreSQL running on Rust - allows you to run jobs (e.g.
sending emails, performing calculations, generating PDFs, etc) "in the
background" so that your HTTP response/application code is not held up. Can be
used with any PostgreSQL-backed application.

### Add the worker to your project:

```
cargo add archimedes
```

### Create tasks and run the worker

The definition of a task consist simply of an async function and a task identifier

```rust
#[derive(Deserialize)]
struct HelloPayload {
    name: String,
}

async fn say_hello(_ctx: WorkerCtx, payload: HelloPayload) -> Result<(), ..> {
    println!("Hello {} !", payload.name);
    Ok(())
}

fn main() {
    archimedes::WorkerOptions::default()
        .concurrency(2)
        .schema("example_simple_worker")
        .define_job("say_hello", say_hello)
        .pg_pool(pg_pool)
        .init()
        .await?
        .run()
        .await?;
}
```

### Schedule a job via SQL

Connect to your database and run the following SQL:

```sql
SELECT archimedes_worker.add_job('say_hello', json_build_object('name', 'Bobby Tables'));
```

### Success!

You should see the worker output `Hello Bobby Tables !`. Gosh, that was fast!

## Features

- Standalone and embedded modes
- Designed to be used both from JavaScript or directly in the database
- <!-- TODO --> Easy to test (recommended: `runTaskListOnce` util) 
- Low latency (typically under 3ms from task schedule to execution, uses
  `LISTEN`/`NOTIFY` to be informed of jobs as they're inserted)
- High performance (uses `SKIP LOCKED` to find jobs to execute, resulting in
  faster fetches)
- Small tasks (uses explicit task names / payloads resulting in minimal
  serialisation/deserialisation overhead)
- Parallel by default
- Adding jobs to same named queue runs them in series
- Automatically re-attempts failed jobs with exponential back-off
- Customisable retry count (default: 25 attempts over ~3 days)
- Crontab-like scheduling feature for recurring tasks (with optional backfill)
- Task de-duplication via unique `job_key`
- Append data to already enqueued jobs with "batch jobs"
- Open source; liberal MIT license
- Executes tasks written in Rust (these can call out to any other language or
  networked service)
- Written natively in Rust
- If you're running really lean, you can run Archimedes in the same Node
  process as your server to keep costs and devops complexity down.

## Status

**NOT** production ready (use it at your own risk).

## Requirements

PostgreSQL 12+
Might work with older versions, but has not been tested.

Note: Postgres 12 is required for the `generated always as (expression)` feature

## Installation

```
cargo add archimedes
```

## Running

`archimedes` manages its own database schema (`archimedes_worker`). Just
point at your database and we handle our own migrations:

## Library usage: running jobs

`archimedes` can be used as a library inside your Rust application.
There are two main use cases for this: running jobs, and queueing jobs. Here are
the APIs for running jobs.

**TODO**

### `run(options: RunnerOptions): Promise<Runner>`

Runs until either stopped by a signal event like `SIGINT` or by calling the
`stop()` method on the resolved object.

The resolved 'Runner' object has a number of helpers on it, see
[Runner object](#runner-object) for more information.

### `runOnce(options: RunnerOptions): Promise<void>`

Equivalent to running the CLI with the `--once` flag. The function will run
until there are no runnable jobs left, and then resolve.

### `runMigrations(options: RunnerOptions): Promise<void>`

Equivalent to running the CLI with the `--schema-only` option. Runs the
migrations and then resolves.

### RunnerOptions

The following options for these methods are available.

- `concurrency`: Maximum number of concurrent job running at the same time. `archimedes` uses an async runtime so it has no problem going beyond your number of CPUs.
- `no_handle_signals`: If set true, we won't install signal handlers and it'll be
  up to you to handle graceful shutdown of the worker if the process receives a
  signal.
- `poll_interval`: Frequency at which we check for new job
- the database is identified through one of these options:
  - `pg_pool`: A `sqlx::PgPool` instance to use
- `schema` can be used to change the default `archimedes_worker` schema to
  something else (equivalent to `--schema` on the CLI)
- `forbidden_flags` see [Forbidden flags](#forbidden-flags) below

One of these must be provided (in order of priority):

- `pg_pool` `sqlx::PgPool` instance
- `database_url` setting
- `DATABASE_URL` envvar <!-- TODO -->

### `Runner` object

The `run` method above resolves to a 'Runner' object that has the following
methods and properties:

- `stop(): Promise<void>` - stops the runner from accepting new jobs, and
  returns a promise that resolves when all the in progress tasks (if any) are
  complete.
- `add_job: AddJobFunction` - see [`add_job`](#addjob).

#### Example: adding a job with `runner.add_job`

<!-- TODO -->

See [`add_job`](#addjob) for more details.

```js
runner.add_job("my_task", my_payload);
```

### `WorkerEvents`

**TODO**

## Library usage: queueing jobs

You can also use the `archimedes_worker` library to queue jobs using one of the
following APIs.

### `make_worker_util(options: WorkerUtilsOptions): Promise<WorkerUtils>`

**TODO**

Useful for adding jobs from within JavaScript in an efficient way.

Runnable example:

We recommend building one instance of WorkerUtils and sharing it as a singleton
throughout your code.

### WorkerUtilsOptions

- exactly one of these keys must be present to determine how to connect to the
  database:
  - `database_url`: A PostgreSQL connection string to the database
    containing the job queue, or
  - `pg_pool`: A `sqlx::PgPool` instance to use
- `schema` can be used to change the default `graphile_worker` schema to
  something else (equivalent to `--schema` on the CLI)

### WorkerUtils

A `WorkerUtils` instance has the following methods:

- `add_job(name: String, payload: serde_json::Value, opts: JobOptions)` - a method you can call
  to enqueue a job, see [addJob](#addjob).
- `migrate()` - a method you can call to update the graphile-worker database
  schema; returns a promise.

### `quick_add_job(options: WorkerUtilsOptions, job: Job): Result<Job>`

If you want to quickly add a job and you don't mind the cost of opening a DB
connection pool and then cleaning it up right away _for every job added_,
there's the `quickAddJob` convenience function. It takes the same options as
`makeWorkerUtils` as the first argument; the remaining arguments are for
[`addJob`](#addjob).

NOTE: you are recommended to use `makeWorkerUtils` instead where possible, but
in one-off scripts this convenience method may be enough.

Runnable example:

```js
const { quickAddJob } = require("graphile-worker");

async function main() {
  await quickAddJob(
    // makeWorkerUtils options
    { connectionString: "postgres:///my_db" },

    // Task identifier
    "calculate-life-meaning",

    // Payload
    { value: 42 },

    // Optionally, add further task spec details here
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

## add\_job

The `add_job` API exists in many places in graphile-worker, but all the instances
have exactly the same call signature. The API is used to add a job to the queue
for immediate or delayed execution. With `job_key` and `job_key_mode` it can also
be used to replace existing jobs.

NOTE: `quick_add_job` is similar to `add_job`, but accepts an additional initial
parameter describing how to connect to the database).

The `add_job` arguments are as follows:

- `identifier`: the name of the task to be executed
- `payload`: an optional JSON-compatible object to give the task more context on
  what it is doing, or a list of these objects in "batch job" mode
- `options`: an optional object specifying:
  - `queue_name`: the queue to run this task under
  - `run_at`: a Date to schedule this task to run in the future
  - `max_attempts`: how many retries should this task get? (Default: 25)
  - `job_key`: unique identifier for the job, used to replace, update or remove
    it later if needed (see
    [Replacing, updating and removing jobs](#replacing-updating-and-removing-jobs));
    can be used for de-duplication (i.e. throttling or debouncing)
  - `job_key_mode`: controls the behavior of `job_key` when a matching job is found
    (see
    [Replacing, updating and removing jobs](#replacing-updating-and-removing-jobs))

Example:

```js
add_job("task_2", json!({ "name": "John" })).await?;
```

### Batch jobs

Not supported for now, if you are interested in porting this to rust, you can express your interested by opening an issue.

## Creating task executors

A task executor is a simple async RUST function which receives as input the job
payload and a collection of helpers. It does the work and then returns. If it
returns then the job is deemed a success and is deleted from the queue (unless
this is a "batch job"). If it throws an error then the job is deemed a failure
and the task is rescheduled using an exponential-backoff algorithm.

**IMPORTANT**: we automatically retry the job if it fails, so it's often
sensible to split large jobs into smaller jobs, this also allows them to run in
parallel resulting in faster execution. This is particularly important for tasks
that are not idempotent (i.e. running them a second time will have extra side
effects) - for example sending emails.

Each task function is passed two arguments:

- `payload` - the payload you passed when calling `add_job`
- `ctx` - an object containing:
  - `job` - the whole job (including `uuid`, `attempts`, etc) - you shouldn't
    need this
  - `pg_pool` - a helper to use to get a database client
  - `add_job` - a helper to schedule a job

### Handling batch jobs

**NOT PORTED**

## More detail on scheduling jobs through SQL

You can schedule jobs directly in the database, e.g. from a trigger or function,
or by calling SQL from your application code. You do this using the
`graphile_worker.add_job` function (or the experimental
`graphile_worker.add_jobs` function for bulk inserts, see below).

NOTE: the [`addJob`](#addjob) JavaScript method simply defers to this underlying
`add_job` SQL function.

`add_job` accepts the following parameters (in this order):

- `identifier` - the only **required** field, indicates the name of the task
  executor to run (omit the `.js` suffix!)
- `payload` - a JSON object with information to tell the task executor what to
  do, or an array of one or more of these objects for "batch jobs" (defaults to
  an empty object)
- `queue_name` - if you want certain tasks to run one at a time, add them to the
  same named queue (defaults to `null`)
- `run_at` - a timestamp after which to run the job; defaults to now.
- `max_attempts` - if this task fails, how many times should we retry it?
  Default: 25.
- `job_key` - unique identifier for the job, used to replace, update or remove
  it later if needed (see
  [Replacing, updating and removing jobs](#replacing-updating-and-removing-jobs));
  can also be used for de-duplication
- `priority` - an integer representing the jobs priority. Jobs are executed in
  numerically ascending order of priority (jobs with a numerically smaller
  priority are run first).
- `flags` - an optional text array (`text[]`) representing a flags to attach to
  the job. Can be used alongside the `forbiddenFlags` option in library mode to
  implement complex rate limiting or other behaviors which requiring skipping
  jobs at runtime (see [Forbidden flags](#forbidden-flags)).
- `job_key_mode` - when `job_key` is specified, this setting indicates what
  should happen when an existing job is found with the same job key:
  - `replace` (default) - all job parameters are updated to the new values,
    including the `run_at` (inserts new job if matching job is locked)
  - `preserve_run_at` - all job parameters are updated to the new values, except
    for `run_at` which maintains the previous value (inserts new job if matching
    job is locked)
  - `unsafe_dedupe` - only inserts the job if no existing job (whether or not it
    is locked or has failed permanently) with matching key is found; does not
    update the existing job

Typically you'll want to set the `identifier` and `payload`:

```sql
SELECT graphile_worker.add_job(
  'send_email',
  json_build_object(
    'to', 'someone@example.com',
    'subject', 'graphile-worker test'
  )
);
```

It's recommended that you use
[PostgreSQL's named parameters](https://www.postgresql.org/docs/current/sql-syntax-calling-funcs.html#SQL-SYNTAX-CALLING-FUNCS-NAMED)
for the other parameters so that you only need specify the arguments you're
using:

```sql
SELECT graphile_worker.add_job('reminder', run_at := NOW() + INTERVAL '2 days');
```

**TIP**: if you want to run a job after a variable number of seconds according
to the database time (rather than the application time), you can use interval
multiplication; see `run_at` in this example:

```sql
SELECT graphile_worker.add_job(
  $1,
  payload := $2,
  queue_name := $3,
  max_attempts := $4,
  run_at := NOW() + ($5 * INTERVAL '1 second')
);
```

**NOTE:** `graphile_worker.add_job(...)` requires database owner privileges to
execute. To allow lower-privileged users to call it, wrap it inside a PostgreSQL
function marked as `SECURITY DEFINER` so that it will run with the same
privileges as the more powerful user that defined it. (Be sure that this
function performs any access checks that are necessary.)

### `add_jobs`

**Experimental**: this API may change in a semver minor release.

For bulk insertion of jobs, we've introduced the `graphile_worker.add_jobs`
function. It accepts the following options:

- `specs` - an array of `graphile_worker.job_spec` objects
- `job_key_preserve_run_at` - an optional boolean detailing if the `run_at`
  should be preserved when the same `job_key` is seen again

The `job_spec` object has the following properties, all of which correspond with
the `add_job` option of the same name above.

- `identifier`
- `payload`
- `queue_name`
- `run_at`
- `max_attempts`
- `job_key`
- `priority`
- `flags`

Note: `job_key_mode='unsafe_dedupe'` is not supported in `add_jobs` - you must
add jobs one at a time using `add_job` to use that. The equivalent of
`job_key_mode='replace'` is enabled by default, to change this to the same
behavior as `job_key_mode='preserve_run_at'` you should set
`job_key_preserve_run_at` to `true`.

### Example: scheduling job from trigger

This snippet creates a trigger function which adds a job to execute
`task_identifier_here` when a new row is inserted into `my_table`.

```sql
CREATE FUNCTION my_table_created() RETURNS trigger AS $$
BEGIN
  PERFORM graphile_worker.add_job('task_identifier_here', json_build_object('id', NEW.id));
  RETURN NEW;
END;
$$ LANGUAGE plpgsql VOLATILE;

CREATE TRIGGER trigger_name AFTER INSERT ON my_table FOR EACH ROW EXECUTE PROCEDURE my_table_created();
```

### Example: one trigger function to rule them all

If your tables are all defined with a single primary key named `id` then you can
define a more convenient dynamic trigger function which can be called from
multiple triggers for multiple tables to quickly schedule jobs.

```sql
CREATE FUNCTION trigger_job() RETURNS trigger AS $$
BEGIN
  PERFORM graphile_worker.add_job(TG_ARGV[0], json_build_object(
    'schema', TG_TABLE_SCHEMA,
    'table', TG_TABLE_NAME,
    'op', TG_OP,
    'id', (CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END)
  ));
  RETURN NEW;
END;
$$ LANGUAGE plpgsql VOLATILE;
```

You might use this trigger like this:

```sql
CREATE TRIGGER send_verification_email
  AFTER INSERT ON user_emails
  FOR EACH ROW
  WHEN (NEW.verified is false)
  EXECUTE PROCEDURE trigger_job('send_verification_email');
CREATE TRIGGER user_changed
  AFTER INSERT OR UPDATE OR DELETE ON users
  FOR EACH ROW
  EXECUTE PROCEDURE trigger_job('user_changed');
CREATE TRIGGER generate_pdf
  AFTER INSERT ON pdfs
  FOR EACH ROW
  EXECUTE PROCEDURE trigger_job('generate_pdf');
CREATE TRIGGER generate_pdf_update
  AFTER UPDATE ON pdfs
  FOR EACH ROW
  WHEN (NEW.title IS DISTINCT FROM OLD.title)
  EXECUTE PROCEDURE trigger_job('generate_pdf');
```

## Replacing, updating and removing jobs

### Replacing/updating jobs

Jobs scheduled with a `job_key` parameter may be replaced/updated by calling
`add_job` again with the same `job_key` value. This can be used for rescheduling
jobs, to ensure only one of a given job is scheduled at a time, or to update
other settings for the job.

For example after the below SQL transaction, the `send_email` job will run only
once, with the payload `'{"count": 2}'`:

```sql
BEGIN;
SELECT graphile_worker.add_job('send_email', '{"count": 1}', job_key := 'abc');
SELECT graphile_worker.add_job('send_email', '{"count": 2}', job_key := 'abc');
COMMIT;
```

In all cases if no match is found then a new job will be created; behavior when
an existing job with the same job key is found is controlled by the
`job_key_mode` setting:

- `replace` (default) - overwrites the unlocked job with the new values. This is
  primarily useful for rescheduling, updating, or **debouncing** (delaying
  execution until there have been no events for at least a certain time period).
  Locked jobs will cause a new job to be scheduled instead.
- `preserve_run_at` - overwrites the unlocked job with the new values, but
  preserves `run_at`. This is primarily useful for **throttling** (executing at
  most once over a given time period). Locked jobs will cause a new job to be
  scheduled instead.
- `unsafe_dedupe` - if an existing job is found, even if it is locked or
  permanently failed, then it won't be updated. This is very dangerous as it
  means that the event that triggered this `add_job` call may not result in any
  action. It is strongly advised you do not use this mode unless you are certain
  you know what you are doing.

The full `job_key_mode` algorithm is roughly as follows:

- If no existing job with the same job key is found:
  - a new job will be created with the new attributes.
- Otherwise, if `job_key_mode` is `unsafe_dedupe`:
  - stop and return the existing job.
- Otherwise, if the existing job is locked:
  - it will have its `key` cleared
  - it will have its attempts set to `max_attempts` to avoid it running again
  - a new job will be created with the new attributes.
- Otherwise, if the existing job has previously failed:
  - it will have its `attempts` reset to 0 (as if it were newly scheduled)
  - it will have its `last_error` cleared
  - it will have all other attributes updated to their new values, including
    `run_at` (even when `job_key_mode` is `preserve_run_at`).
- Otherwise, if `job_key_mode` is `preserve_run_at`:
  - the job will have all its attributes except for `run_at` updated to their
    new values.
- Otherwise:
  - the job will have all its attributes updated to their new values.

### Removing jobs

Pending jobs may also be removed using `job_key`:

```sql
SELECT graphile_worker.remove_job('abc');
```

### `job_key` caveats

**IMPORTANT**: jobs that complete successfully are deleted, there is no
permanent `job_key` log, i.e. `remove_job` on a completed `job_key` is a no-op
as no row exists.

**IMPORTANT**: the `job_key` is treated as universally unique (whilst the job is
pending/failed), so you can update a job to have a completely different
`task_identifier` or `payload`. You must be careful to ensure that your
`job_key` is sufficiently unique to prevent you accidentally replacing or
deleting unrelated jobs by mistake; one way to approach this is to incorporate
the `task_identifier` into the `job_key`.

**IMPORTANT**: If a job is updated using `add_job` when it is currently locked
(i.e. running), a second job will be scheduled separately (unless
`job_key_mode = 'unsafe_dedupe'`), meaning both will run.

**IMPORTANT**: calling `remove_job` for a locked (i.e. running) job will not
actually remove it, but will prevent it from running again on failure.

## Administration functions

### Complete jobs

SQL: `SELECT * FROM graphile_worker.complete_jobs(ARRAY[7, 99, 38674, ...])`;

RUST : TODO

Marks the specified jobs (by their ids) as if they were completed, assuming they
are not locked. Note that completing a job deletes it. You may mark failed and
permanently failed jobs as completed if you wish. The deleted jobs will be
returned (note that this may be fewer jobs than you requested).

### Permanently fail jobs

SQL:
`SELECT * FROM graphile_worker.permanently_fail_jobs(ARRAY[7, 99, 38674, ...], 'Enter reason here')`;

RUST : TODO

Marks the specified jobs (by their ids) as failed permanently, assuming they are
not locked. This means setting their `attempts` equal to their `max_attempts`.
The updated jobs will be returned (note that this may be fewer jobs than you
requested).

### Rescheduling jobs

SQL:

```sql
SELECT * FROM graphile_worker.reschedule_jobs(
  ARRAY[7, 99, 38674, ...],
  run_at := NOW() + interval '5 minutes',
  priority := 5,
  attempts := 5,
  max_attempts := 25
);
```

RUST : TODO

Updates the specified scheduling properties of the jobs (assuming they are not
locked). All of the specified options are optional, omitted or null values will
left unmodified.

This method can be used to postpone or advance job execution, or to schedule a
previously failed or permanently failed job for execution. The updated jobs will
be returned (note that this may be fewer jobs than you requested).

## Recurring tasks (crontab)

Archimedes Worker supports triggering recurring tasks according to a cron-like
schedule. This is designed for recurring tasks such as sending a weekly email,
running database maintenance tasks every day, performing data roll-ups hourly,
downloading external data every 20 minutes, etc.

Archimedes Worker's crontab support:

- guarantees (thanks to ACID-compliant transactions) that no duplicate task
  schedules will occur
- can backfill missed jobs if desired (e.g. if the Worker wasn't running when
  the job was due to be scheduled)
- schedules tasks using Graphile Worker's regular job queue, so you get all the
  regular features such as exponential back-off on failure.
- works reliably even if you're running multiple workers (see "Distributed
  crontab" below)

**NOTE**: It is not intended that you add recurring tasks for each of your
individual application users, instead you should have relatively few recurring
tasks, and those tasks can create additional jobs for the individual users (or
process multiple users) if necessary.

Tasks are by default read from a `crontab` file next to the `tasks/` folder (but
this is configurable in library mode). Please note that our syntax is not 100%
compatible with cron's, and our task payload differs. We only handle timestamps
in UTC. The following diagram details the parts of a Graphile Worker crontab
schedule:

```crontab
# ┌───────────── UTC minute (0 - 59)
# │ ┌───────────── UTC hour (0 - 23)
# │ │ ┌───────────── UTC day of the month (1 - 31)
# │ │ │ ┌───────────── UTC month (1 - 12)
# │ │ │ │ ┌───────────── UTC day of the week (0 - 6) (Sunday to Saturday)
# │ │ │ │ │ ┌───────────── task (identifier) to schedule
# │ │ │ │ │ │    ┌────────── optional scheduling options
# │ │ │ │ │ │    │     ┌────── optional payload to merge
# │ │ │ │ │ │    │     │
# │ │ │ │ │ │    │     │
# * * * * * task ?opts {payload}
```

Comment lines start with a `#`.

For the first 5 fields we support an explicit numeric value, `*` to represent
all valid values, `*/n` (where `n` is a positive integer) to represent all valid
values divisible by `n`, range syntax such as `1-5`, and any combination of
these separated by commas.

The task identifier should match the following regexp
`/^[_a-zA-Z][_a-zA-Z0-9:_-]*$/` (namely it should start with an alphabetic
character and it should only contain alphanumeric characters, colon, underscore
and hyphen). It should be the name of one of your Graphile Worker tasks.

The `opts` must always be prefixed with a `?` if provided and details
configuration for the task such as what should be done in the event that the
previous event was not scheduled (e.g. because the Worker wasn't running).
Options are specified using HTTP query string syntax (with `&` separator).

Currently we support the following `opts`:

- `id=UID` where UID is a unique alphanumeric case-sensitive identifier starting
  with a letter - specify an identifier for this crontab entry; by default this
  will use the task identifier, but if you want more than one schedule for the
  same task (e.g. with different payload, or different times) then you will need
  to supply a unique identifier explicitly.
- `fill=t` where `t` is a "time phrase" (see below) - backfill any entries from
  the last time period `t`, for example if the worker was not running when they
  were due to be executed (by default, no backfilling).
- `max=n` where `n` is a small positive integer - override the `max_attempts` of
  the job.
- `queue=name` where `name` is an alphanumeric queue name - add the job to a
  named queue so it executes serially.
- `priority=n` where `n` is a relatively small integer - override the priority
  of the job.

**NOTE**: changing the identifier (e.g. via `id`) can result in duplicate
executions, so we recommend that you explicitly set it and never change it.

**NOTE**: using `fill` will not backfill new tasks, only tasks that were
previously known.

**NOTE**: the higher you set the `fill` parameter, the longer the worker startup
time will be; when used you should set it to be slightly larger than the longest
period of downtime you expect for your worker.

Time phrases are comprised of a sequence of number-letter combinations, where
the number represents a quantity and the letter represents a time period, e.g.
`5d` for `five days`, or `3h` for `three hours`; e.g. `4w3d2h1m` represents
`4 weeks, 3 days, 2 hours and 1 minute` (i.e. a period of 44761 minutes). The
following time periods are supported:

- `s` - one second (1000 milliseconds)
- `m` - one minute (60 seconds)
- `h` - one hour (60 minutes)
- `d` - one day (24 hours)
- `w` - one week (7 days)

The `payload` is a JSON5 object; it must start with a `{`, must not contain
newlines or carriage returns (`\n` or `\r`), and must not contain trailing
whitespace. It will be merged into the default crontab payload properties.

Each crontab job will have a JSON object payload containing the key `_cron` with
the value being an object with the following entries:

- `ts` - ISO8601 timestamp representing when this job was due to execute
- `backfilled` - true if the task was "backfilled" (i.e. it wasn't scheduled on
  time), false otherwise

### Distributed crontab

**TL;DR**: when running identical crontabs on multiple workers no special action
is necessary - it Just Works :tm:

When you run multiple workers with the same crontab files then the first worker
that attempts to queue a particular cron job will succeed and the other workers
will take no action - this is thanks to SQL ACID-compliant transactions and our
`known_crontabs` lock table.

If your workers have different crontabs then you must be careful to ensure that
the cron items each have unique identifiers; the easiest way to do this is to
specify the identifiers yourself (see the `id=` option above). Should you forget
to do this then for any overlapping timestamps for items that have the same
derived identifier one of the cron tasks will schedule but the others will not.

### Crontab examples

The following schedules the `send_weekly_email` task at 4:30am (UTC) every
Monday:

```
30 4 * * 1 send_weekly_email
```

The following does similar, but also will backfill any tasks over the last two
days (`2d`), sets max attempts to `10` and merges in `{"onboarding": false}`
into the task payload:

```
30 4 * * 1 send_weekly_email ?fill=2d&max=10 {onboarding:false}
```

The following triggers the `rollup` task every 4 hours on the hour:

```
0 */4 * * * rollup
```

### Limiting backfill

When you ask Graphile Worker to backfill jobs, it will do so for all jobs
matching that specification that should have been scheduled over the backfill
period. Other than the period itself, you cannot place limits on the backfilling
(for example, you cannot say "backfill at most one job" or "only backfill if the
next job isn't due within the next 3 hours"); this is because we've determined
that there's many situations (back-off, overloaded worker, serially executed
jobs, etc.) in which the result of this behaviour might result in outcomes that
the user did not expect.

If you need these kinds of constraints on backfilled jobs, you should implement
them _at runtime_ (rather than at scheduling time) in the task executor itself,
which could use the `payload._cron.ts` property to determine whether execution
should continue or not.

## Forbidden flags

When a job is created (or updated via `job_key`), you may set its `flags` to a
list of strings. When the worker is run in library mode, you may pass the
`forbidden_flags` option to indicate that jobs with any of the given flags should
not be executed.

The `forbidden_flags` option can be:

- null
- an array of strings
- a function returning null or an array of strings
- an (async) function returning a promise that resolve to null or an array of
  strings

If `forbidden_flags` is a function, `graphile-worker` will invoke it each time a
worker looks for a job to run, and will skip over any job that has any flag
returned by your function. You should ensure that `forbidden_flags` resolves
quickly; it's advised that you maintain a cache you update periodically (e.g.
once a second) rather than always calculating on the fly, or use pub/sub or a
similar technique to maintain the forbidden flags list.

For an example of how this can be used to achieve rate-limiting logic

## Rationality checks

We recommend that you limit `queue_name`, `task_identifier` and `job_key` to
printable ASCII characters.

- `queue_name` can be at most 128 characters long
- `task_identifier` can be at most 128 characters long
- `job_key` can be at most 512 characters long
- `schema` should be reasonable; max 32 characters is preferred. Defaults to
  `graphile_worker` (15 chars)

## Uninstallation

To delete the worker code and all the tasks from your database, just run this
one SQL statement:

```sql
DROP SCHEMA archimedes_worker CASCADE;
```

## Performance

`archimedes_worker` is not intended to replace extremely high performance
dedicated job queues, it's intended to be a very easy way to get a reasonably
performant job queue up and running with rust and PostgreSQL. But this
doesn't mean it's a slouch by any means - it achieves an average latency from
triggering a job in one process to executing it in another of under 3ms, and a
12-core database server can queue around 99,600 jobs per second and can process
around 11,800 jobs per second.

`archimedes_worker` is horizontally scalable. Each instance has a customisable
worker pool, this pool defaults to size 1 (only one job at a time on this
worker) but depending on the nature of your tasks (i.e. assuming they're not
compute-heavy) you will likely want to set this higher to benefit from Node.js'
concurrency. If your tasks are compute heavy you may still wish to set it higher
and then using Node's `child_process` (or Node v11's `worker_threads`) to share
the compute load over multiple cores without significantly impacting the main
worker's runloop. Note, however, that Graphile Worker is limited by the
performance of the underlying Postgres database, and when you hit this limit
performance will start to go down (rather than up) as you add more workers.

### perfTest results:

TODO 

## Exponential-backoff

We currently use the formula `exp(least(10, attempt))` to determine the delays
between attempts (the job must fail before the next attempt is scheduled, so the
total time elapsed may be greater depending on how long the job runs for before
it fails). This seems to handle temporary issues well, after ~4 hours attempts
will be made every ~6 hours until the maximum number of attempts is achieved.
The specific delays can be seen below:

```
select
  attempt,
  exp(least(10, attempt)) * interval '1 second' as delay,
  sum(exp(least(10, attempt)) * interval '1 second') over (order by attempt asc) total_delay
from generate_series(1, 24) as attempt;

 attempt |      delay      |   total_delay
---------+-----------------+-----------------
       1 | 00:00:02.718282 | 00:00:02.718282
       2 | 00:00:07.389056 | 00:00:10.107338
       3 | 00:00:20.085537 | 00:00:30.192875
       4 | 00:00:54.598150 | 00:01:24.791025
       5 | 00:02:28.413159 | 00:03:53.204184
       6 | 00:06:43.428793 | 00:10:36.632977
       7 | 00:18:16.633158 | 00:28:53.266135
       8 | 00:49:40.957987 | 01:18:34.224122
       9 | 02:15:03.083928 | 03:33:37.308050
      10 | 06:07:06.465795 | 09:40:43.773845
      11 | 06:07:06.465795 | 15:47:50.239640
      12 | 06:07:06.465795 | 21:54:56.705435
      13 | 06:07:06.465795 | 28:02:03.171230
      14 | 06:07:06.465795 | 34:09:09.637025
      15 | 06:07:06.465795 | 40:16:16.102820
      16 | 06:07:06.465795 | 46:23:22.568615
      17 | 06:07:06.465795 | 52:30:29.034410
      18 | 06:07:06.465795 | 58:37:35.500205
      19 | 06:07:06.465795 | 64:44:41.966000
      20 | 06:07:06.465795 | 70:51:48.431795
      21 | 06:07:06.465795 | 76:58:54.897590
      22 | 06:07:06.465795 | 83:06:01.363385
      23 | 06:07:06.465795 | 89:13:07.829180
      24 | 06:07:06.465795 | 95:20:14.294975
```

## What if something goes wrong?

If a job throws an error, the job is failed and scheduled for retries with
exponential back-off. We use async/await so assuming you write your task code
well all errors should be cascaded down automatically.

**TODO**
If the worker is terminated (`SIGTERM`, `SIGINT`, etc), it
[triggers a graceful shutdown](https://github.com/graphile/worker/blob/3540df5ab4eb73f846d54959fdfad07897b616f0/src/main.ts#L39-L66) -
i.e. it stops accepting new jobs, waits for the existing jobs to complete, and
then exits. If you need to restart your worker, you should do so using this
graceful process.

If the worker completely dies unexpectedly (e.g. `panic`, 
`SIGKILL`) then the jobs that that worker was executing remain locked for at
least 4 hours. Every 8-10 minutes a worker will sweep for jobs that have been
locked for more than 4 hours and will make them available to be processed again
automatically. If you run many workers, each worker will do this, so it's likely
that jobs will be released closer to the 4 hour mark. You can unlock jobs
earlier than this by clearing the `locked_at` and `locked_by` columns on the
relevant tables.

If the worker schema has not yet been installed into your database, the
following error may appear in your PostgreSQL server logs. This is completely
harmless and should only appear once as the worker will create the schema for
you.

```
ERROR: relation "archimedes_worker.migrations" does not exist at character 16
STATEMENT: select id from "graphile_worker".migrations order by id desc limit 1;
```

### Error codes

- `GWBKM` - Invalid `job_key_mode` value, expected `'replace'`,
  `'preserve_run_at'` or `'unsafe_dedupe'`.


## Thanks for reading!

If this project helps you out, please
[sponsor ongoing development](https://www.graphile.org/sponsor/).
