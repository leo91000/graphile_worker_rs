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
use serde::{Deserialize, Serialize};
use archimedes::{task, WorkerContext};

#[derive(Deserialize, Serialize)]
struct HelloPayload {
    name: String,
}

#[task]
async fn say_hello(payload: HelloPayload, _ctx: WorkerContext) -> Result<(), ()> {
    println!("Hello {} !", payload.name);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    archimedes::WorkerOptions::default()
        .concurrency(2)
        .schema("example_simple_worker")
        .define_job(say_hello)
        .pg_pool(pg_pool)
        .init()
        .await?
        .run()
        .await?;

    Ok(())
}
```

### Schedule a job via SQL

Connect to your database and run the following SQL:

```sql
SELECT archimedes_worker.add_job('say_hello', json_build_object('name', 'Bobby Tables'));
```

### Schedule a job via RUST

```rust
#[tokio::main]
async fn main() -> Result<(), ()> {
    // ...
    let helpers = worker.create_helpers();

    // Using add_job forces the payload to be same struct defined in our type
    helpers.add_job::<say_hello>(
        HelloPayload { name: "world".to_string() },
        Default::default(),
    ).await.unwrap();

    // You can also use `add_raw_job` if you don't have access to the task, or don't care about end 2 end safety
    helpers.add_raw_job("say_hello", serde_json::json!({ "message": "world" }), Default::default()).await.unwrap();

    Ok(())
}
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
point at your database and we handle our own migrations.

