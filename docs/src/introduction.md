# Introduction

Graphile Worker RS is a PostgreSQL-backed job queue for Rust applications. It
lets an application store durable work in PostgreSQL, then process that work in
background workers instead of blocking request handling or other latency
sensitive paths.

Use it for work such as sending email, generating documents, running
calculations, synchronizing data, cleaning up records, or moving jobs produced
by PostgreSQL triggers and functions into Rust code. Jobs are stored in
PostgreSQL, task handlers run in Rust, and the worker coordinates fetching,
execution, retries, queues, and scheduling.

## Relationship to Graphile Worker

Graphile Worker RS is based on the Node.js
[Graphile Worker](https://github.com/graphile/worker) project. The core model is
the same: PostgreSQL is the durable queue, workers claim available jobs, task
identifiers choose the handler, and job metadata controls scheduling, priority,
attempts, and queue behavior.

This project is not a Node wrapper. It is a Rust rewrite with Rust APIs,
workspace crates, feature flags, and async runtime integration. The root crate is
`graphile_worker`, and the workspace contains focused crates for task handlers,
job specifications, context, migrations, cron parsing and running, lifecycle
hooks, recovery, database access, runtime adapters, the CLI, and admin tooling.

The Rust implementation is mostly compatible with the original Graphile Worker
model, so it can be used alongside the Node.js worker where the shared database
schema and job contracts match. One important implementation difference is worker
identity: the Rust worker uses one worker id per worker instance, and jobs are
processed by the enabled async runtime.

## What It Solves

Graphile Worker RS is useful when PostgreSQL is already part of your system and
you want background processing without operating a separate queue service. It
provides:

- durable job storage in PostgreSQL
- efficient job claiming with PostgreSQL locking
- low-latency wakeups through PostgreSQL notifications
- typed Rust task handlers through `TaskHandler`
- scheduling from Rust through `WorkerUtils` or from SQL through
  `graphile_worker.add_job`
- delayed jobs, priorities, attempts, job keys, and serial queues
- cron-like recurring jobs through the crontab crates
- batch job insertion and batch task handling
- configurable worker concurrency
- graceful shutdown behavior for in-flight work
- optional worker recovery for jobs left locked by failed workers
- lifecycle hooks, tracing integration, CLI commands, and admin UI crates for
  operational visibility

The default feature set uses Tokio, rustls, and the SQLx PostgreSQL driver.
Feature flags also expose async-std, native TLS, the tokio-postgres driver, and
OpenTelemetry compatibility groups.

## Guide and API Reference

This mdBook is the narrative guide. Read it when you want to understand how the
pieces fit together, choose configuration, follow a task-oriented workflow, or
prepare a worker for production.

Start with [Getting Started](getting-started/index.md) to install the crate,
define a first `TaskHandler`, register it with `WorkerOptions`, and run a worker.
Then use [Core Concepts](concepts/index.md) for the mental model around tasks,
workers, scheduling, queues, and the database schema.

Use [Configuration](configuration/index.md) when selecting runtime, TLS, database
driver, shutdown, recovery, and application state options. Use
[Guides](guides/index.md) for focused workflows such as scheduling jobs, batch
jobs, cron jobs, local queue behavior, worker recovery, lifecycle hooks, and job
management. Use [Operations](operations/index.md) for the CLI, admin UI,
migrations, deployment, and observability topics.

Use [docs.rs](https://docs.rs/graphile_worker/) when you need generated Rust API
documentation: exact method signatures, trait bounds, enum variants, struct
fields, feature-gated items, and crate-level rustdoc. The
[API Reference](reference/api.md) page links to the generated docs for the main
crate and supporting crates, while [Crate Map](reference/crates.md) explains what
each workspace crate is for.

For executable examples, start with `examples/simple.rs`, then look at
`examples/crontab.rs`, `examples/batch_add_jobs.rs`, `examples/local_queue.rs`,
and the hook examples. The integration tests under `tests/` are also useful when
you need precise behavior for scheduling, queues, worker utilities, migrations,
recovery, and runtime driver paths.
