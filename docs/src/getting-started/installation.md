# Installation

Graphile Worker RS is published as the `graphile_worker` crate. It needs a
PostgreSQL database and one supported async runtime in your application.

## Add the Crate

For the default setup, add the main crate:

```bash
cargo add graphile_worker
```

Or add it to `Cargo.toml` directly:

```toml
[dependencies]
graphile_worker = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The default feature set enables:

- `runtime-tokio`
- `tls-rustls`
- `driver-sqlx`

This is the recommended starting point for Tokio applications using SQLx and
rustls TLS.

## Tokio

Tokio is enabled by default, so most applications only need to add Tokio with
the runtime features they use:

```toml
[dependencies]
graphile_worker = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

A typical Tokio entrypoint looks like this:

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_worker().await
}
```

## async-std

To use async-std, disable default features and enable `runtime-async-std`.
Because the default SQLx driver and rustls TLS are also disabled when default
features are disabled, enable them explicitly if you still want that setup:

```toml
[dependencies]
graphile_worker = { version = "0.13", default-features = false, features = [
  "runtime-async-std",
  "tls-rustls",
  "driver-sqlx",
] }
async-std = { version = "1", features = ["attributes"] }
```

Applications using the `#[async_std::main]` macro need async-std's
`attributes` feature:

```rust,ignore
#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_worker().await
}
```

## Driver and TLS Features

The SQLx PostgreSQL driver is enabled by default through `driver-sqlx`. The
crate also exposes `driver-tokio-postgres` for Tokio applications.

```toml
[dependencies]
graphile_worker = { version = "0.13", default-features = false, features = [
  "runtime-tokio",
  "driver-tokio-postgres",
] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The `driver-tokio-postgres` feature enables `runtime-tokio` and is tested only
with the Tokio runtime. Use `driver-sqlx` for async-std.

TLS is selected separately from the database driver. The available TLS feature
flags are:

- `tls-rustls`
- `tls-native-tls`

When you disable default features, choose the runtime, database driver, and TLS
features you need explicitly.

## DATABASE_URL

Graphile Worker RS connects to PostgreSQL. Local tests and examples in this
repository use a standard PostgreSQL connection string through `DATABASE_URL`:

```bash
export DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres'
```

Use the same URL form for your own database, changing the username, password,
host, port, and database name as needed.

For a worker setup example that creates a SQLx pool and registers jobs, continue
to [Quick Start](quick-start.md). For a complete feature list, see
[Feature Flags](../reference/features.md).
