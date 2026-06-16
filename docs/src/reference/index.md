# Reference

Use this section when you already know the concept you need and want exact
package, feature, API, compatibility, or troubleshooting details.

The guide pages explain how to build common workflows. The reference pages are
for looking up the supported surface area and checking which crate, feature flag,
or documentation source applies to your use case.

## docs.rs and this guide

The generated Rust API documentation is published on
[docs.rs](https://docs.rs/graphile_worker). Use docs.rs when you need item-level
details for public Rust types, traits, builders, and re-exports from the
`graphile_worker` crate.

Use this mdBook guide when you need operational or architectural context:

- [Worker options](../configuration/worker-options.md) explains how to assemble
  a worker configuration.
- [Runtime, TLS, and drivers](../configuration/runtime-drivers.md) explains how
  feature selections affect database access.
- [Migrations](../operations/migrations.md) covers schema management.
- [Observability](../operations/observability.md) covers tracing and runtime
  visibility.
- [Troubleshooting](troubleshooting.md) collects common failure modes.

For example, look up the exact methods and trait bounds on docs.rs, then return
to the guide for the surrounding setup:

```rust,ignore
use graphile_worker::{TaskHandler, WorkerOptions};

// Check docs.rs for the exact public API of each type.
// Check the guide for configuration examples and operational context.
```

## Public crate surface

The top-level `graphile_worker` crate re-exports the main types used by
applications:

- `Worker` for running workers.
- `WorkerOptions`, `CronInput`, and `WorkerBuildError` for configuration.
- `WorkerUtils` for job management helpers.
- `TaskHandler` and related task handling types.
- Job and job specification types from the job crates.
- Context, database, lifecycle hook, shutdown signal, and cron types.
- `LocalQueue` and its configuration and error types.
- Worker recovery types such as `WorkerRecoveryConfig` and
  `SweepStaleWorkersOptions`.

The workspace also contains smaller crates for specific responsibilities. See
[Crate Map](crates.md) when you need to know which package owns a type or
feature area.

## Feature flags

The default `graphile_worker` feature set enables:

```toml
default = ["runtime-tokio", "tls-rustls", "driver-sqlx"]
```

The visible feature groups are:

- Runtime: `runtime-tokio`, `runtime-async-std`.
- TLS: `tls-rustls`, `tls-native-tls`.
- Database driver: `driver-sqlx`, `driver-tokio-postgres`.
- OpenTelemetry compatibility: `opentelemetry_0_30`,
  `opentelemetry_0_31`, `opentelemetry_0_32`.

Start with the defaults unless you have a specific runtime, TLS, database
driver, or OpenTelemetry version requirement. See [Feature Flags](features.md)
and [Runtime, TLS, and Drivers](../configuration/runtime-drivers.md) before
changing them.

## Compatibility checklist

Before changing a dependency or feature set, check the compatibility page for
the constraints that matter across the workspace:

- Runtime feature enabled.
- Database driver feature enabled.
- TLS backend enabled.
- OpenTelemetry compatibility feature matching your tracing stack.

See [Compatibility](compatibility.md) for the reference checklist.

## Troubleshooting entry points

For setup or runtime issues, start with the page closest to the symptom:

- Worker configuration problems: [Worker Options](../configuration/worker-options.md).
- Database schema or migration problems: [Migrations](../operations/migrations.md).
- Shutdown behavior: [Shutdown](../configuration/shutdown.md).
- Application state and extensions: [Application State and Extensions](../configuration/app-state.md).
- General reference issues: [Troubleshooting](troubleshooting.md).

When the issue is about a specific Rust type or trait, use the
[API Reference](api.md) page as the bridge to docs.rs.
