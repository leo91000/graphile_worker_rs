# Graphile Worker RS

[![Codecov](https://codecov.io/github/leo91000/graphile_worker_rs/coverage.svg?branch=main)](https://codecov.io/gh/leo91000/graphile_worker_rs)
[![Crates.io](https://badgen.net/crates/v/graphile_worker)](https://crates.io/crates/graphile_worker)
[![Documentation](https://img.shields.io/docsrs/graphile_worker)](https://docs.rs/graphile_worker/)
[![MIT License](https://shields.io/badge/license-MIT-blue)](https://github.com/leo91000/graphile_worker_rs/blob/main/LICENSE.md)

A PostgreSQL-backed job queue for Rust applications, based on
[Graphile Worker](https://github.com/graphile/worker).

Graphile Worker RS lets you run durable background jobs from Rust while keeping
PostgreSQL as the queue and coordination backend. It supports typed task
handlers, delayed jobs, priorities, queues, retries, cron jobs, graceful
shutdown, optional dead-worker recovery, lifecycle hooks, CLI tooling, and an
admin UI.

## Documentation

- [Guide](https://leo91000.github.io/graphile_worker_rs/) - task-oriented
  documentation, examples, configuration, operations, and troubleshooting.
- [API reference](https://docs.rs/graphile_worker/) - generated Rust API docs.
- [Examples](https://github.com/leo91000/graphile_worker_rs/tree/main/examples) -
  runnable repository examples.

## Installation

```bash
cargo add graphile_worker
```

Tokio, rustls, and SQLx are enabled by default:

```toml
[dependencies]
graphile_worker = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
```

For async-std, tokio-postgres, native TLS, or OpenTelemetry compatibility
features, see the
[feature flags guide](https://leo91000.github.io/graphile_worker_rs/reference/features.html).

## Quick Start

```rust,ignore
use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext, WorkerOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    to: String,
}

impl TaskHandler for SendEmail {
    const IDENTIFIER: &'static str = "send_email";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Sending email to {}", self.to);
        Ok::<(), String>(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = WorkerOptions::default()
        .database_url("postgres://postgres:password@localhost/mydb")
        .concurrency(5)
        .schema("graphile_worker")
        .define_job::<SendEmail>()
        .init()
        .await?;

    worker
        .create_utils()
        .add_job(
            SendEmail {
                to: "user@example.com".to_string(),
            },
            JobSpec::default(),
        )
        .await?;

    worker.run().await?;
    Ok(())
}
```

For a complete walkthrough, read
[Quick Start](https://leo91000.github.io/graphile_worker_rs/getting-started/quick-start.html)
and
[First Worker](https://leo91000.github.io/graphile_worker_rs/getting-started/first-worker.html).

## Highlights

- PostgreSQL-backed durable queue with `SKIP LOCKED` job claiming.
- Low-latency wakeups through PostgreSQL `LISTEN`/`NOTIFY`.
- Typed Rust task handlers and batch task handlers.
- Delayed jobs, priorities, retry attempts, job keys, and named queues.
- Cron-like recurring jobs through the crontab crates.
- Worker utilities for adding, rescheduling, completing, failing, cleaning up,
  and recovering jobs.
- Optional local queue for batch-fetching jobs and reducing database round trips.
- Graceful shutdown and optional heartbeat-based dead-worker recovery.
- Lifecycle hooks for logging, metrics, validation, and custom policies.
- CLI and admin UI crates for operational workflows.

## Compatibility

This is a Rust rewrite of Graphile Worker. It is mostly compatible with the
Node.js version and can run side by side with it when the shared PostgreSQL
schema and job contracts match.

One notable implementation difference: the Node.js version uses one worker id
per process, while this Rust implementation uses one worker id per worker
instance and processes jobs through the enabled async runtime.

See
[Compatibility](https://leo91000.github.io/graphile_worker_rs/reference/compatibility.html)
for details.

## Development

Common commands:

```bash
just lint
just test-docker
just docs
just docs-serve 3002
```

Before committing, run:

```bash
just lint
just test-docker
```

See
[CONTRIBUTING.md](https://github.com/leo91000/graphile_worker_rs/blob/main/CONTRIBUTING.md)
for contribution details.

## License

MIT. See
[LICENSE.md](https://github.com/leo91000/graphile_worker_rs/blob/main/LICENSE.md).
