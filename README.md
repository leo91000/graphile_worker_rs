# Graphile Worker RS

[![Codecov](https://codecov.io/github/leo91000/graphile_worker_rs/coverage.svg?branch=main)](https://codecov.io/gh/leo91000/graphile_worker_rs)
[![Crates.io](https://img.shields.io/crates/v/graphile-worker.svg)](https://crates.io/crates/graphile-worker)
[![Documentation](https://img.shields.io/docsrs/graphile_worker)](https://docs.rs/graphile_worker/)

Rewrite of [Graphile Worker](https://github.com/graphile/worker) in Rust. If you like this library go sponsor [Benjie](https://github.com/benjie) project, all research has been done by him, this library is only a rewrite in Rust 🦀.
The port should mostly be compatible with `graphile-worker` (meaning you can run it side by side with Node.JS).

The following differs from `Graphile Worker` :
- No support for batch job (I don't need it personnally, but if this is not your case, create an issue and I will see what I can do)
- In `Graphile Worker`, each process has it's worker_id. In rust there is only one worker_id, then jobs are processed in your async runtime thread.

Job queue for PostgreSQL running on Rust - allows you to run jobs (e.g.
sending emails, performing calculations, generating PDFs, etc) "in the
background" so that your HTTP response/application code is not held up. Can be
used with any PostgreSQL-backed application.

### Add the worker to your project:

```bash,ignore
cargo add graphile_worker
```

### Create tasks and run the worker

The definition of a task consist simply of an async function and a task identifier

```rust,ignore
use serde::{Deserialize, Serialize};
use graphile_worker::{WorkerContext, TaskHandler};

#[derive(Deserialize, Serialize)]
struct SayHello {
    message: String,
}

impl TaskHandler for SayHello {
    const IDENTIFIER: &'static str = "say_hello";

    async fn run(self, _ctx: WorkerContext) -> Result<(), ()> {
        println!("Hello {} !", self.message);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    graphile_worker::WorkerOptions::default()
        .concurrency(2)
        .schema("example_simple_worker")
        .define_job::<SayHello>()
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

```sql,ignore
SELECT graphile_worker.add_job('say_hello', json_build_object('name', 'Bobby Tables'));
```

### Schedule a job via RUST

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), ()> {
    // ...
    let utils = worker.create_utils();

    // Using add_job
    utils.add_job(
        SayHello { name: "Bobby Tables".to_string() },
        Default::default(),
    ).await.unwrap();

    // You can also use `add_raw_job` if you don't have access to the task, or don't care about end 2 end safety
    utils.add_raw_job("say_hello", serde_json::json!({ "name": "Bobby Tables" }), Default::default()).await.unwrap();

    Ok(())
}
```

### App state

You can provide app state through `extension` :

```rust,ignore
use serde::{Deserialize, Serialize};
use graphile_worker::{WorkerContext, TaskHandler};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct AppState {
    run_count: Arc<AtomicUsize>,
}

#[derive(Deserialize, Serialize)]
pub struct CounterTask;

impl TaskHandler for CounterTask {
    const IDENTIFIER: &'static str = "counter_task";

    async fn run(self, ctx: WorkerContext) {
        let app_state = ctx.extensions().get::<AppState>().unwrap();
        let run_count = app_state.run_count.fetch_add(1, SeqCst);
        println!("Run count: {run_count}");
    }
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    graphile_worker::WorkerOptions::default()
        .concurrency(2)
        .schema("example_simple_worker")
        .add_extension(AppState {
            run_count: Arc::new(AtomicUsize::new(0)),
        })
        .define_job::<CounterTask>()
        .pg_pool(pg_pool)
        .init()
        .await?
        .run()
        .await?;

    Ok(())
}
```

## Features

- Standalone and embedded modes
- Designed to be used both from JavaScript or directly in the database
- Easy to test (recommended: `run_once` util) 
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
- If you're running really lean, you can run Graphile Worker in the same Rust
  process as your server to keep costs and devops complexity down.

## Status

Production ready but the API may be rough around the edges and might change.

## Requirements

PostgreSQL 12+
Might work with older versions, but has not been tested.

Note: Postgres 12 is required for the `generated always as (expression)` feature

## Installation

```bash,ignore
cargo add graphile_worker
```

## Running

`graphile_worker` manages its own database schema (`graphile_worker_worker`). Just
point at your database and we handle our own migrations.

