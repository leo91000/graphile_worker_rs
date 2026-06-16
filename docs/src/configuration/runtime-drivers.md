# Runtime, TLS, and Drivers

Graphile Worker RS exposes three separate feature choices for database access:

- an async runtime feature
- a TLS backend feature
- a PostgreSQL driver feature

The default crate features enable the common path:

```toml
[dependencies]
graphile_worker = "0.13"
```

This is equivalent to enabling `runtime-tokio`, `tls-rustls`, and
`driver-sqlx`.

## Feature Groups

Runtime features choose the async runtime used by the worker internals:

| Feature | Notes |
| --- | --- |
| `runtime-tokio` | Default runtime. Required by `driver-tokio-postgres`. |
| `runtime-async-std` | Supported with the SQLx driver. |

TLS features choose the TLS implementation passed through to the database
crates:

| Feature | Notes |
| --- | --- |
| `tls-rustls` | Default TLS backend. |
| `tls-native-tls` | Native TLS backend. |

Driver features choose which PostgreSQL client integration is compiled:

| Feature | Notes |
| --- | --- |
| `driver-sqlx` | Default driver. Enables the optional `sqlx` dependency. |
| `driver-tokio-postgres` | Enables the tokio-postgres based driver path and also enables `runtime-tokio`. |

## Valid Combinations

Use one runtime, one TLS backend when your driver needs TLS, and one driver.
The combinations exercised by the repository test matrix are:

| Runtime | Driver | TLS in tested command | Status |
| --- | --- | --- | --- |
| `runtime-tokio` | `driver-sqlx` | `tls-rustls` | Default tested path. |
| `runtime-async-std` | `driver-sqlx` | `tls-rustls` | Tested SQLx async-std path. |
| `runtime-tokio` | `driver-tokio-postgres` | Not added by the matrix command | Tested tokio-postgres path. |

`driver-tokio-postgres` is Tokio-only. The repository runtime test command
rejects `driver-tokio-postgres` with any runtime other than `runtime-tokio`.

## Cargo Examples

Use defaults unless you have a reason to choose another runtime or driver:

```toml
[dependencies]
graphile_worker = "0.13"
```

To make the default feature set explicit:

```toml
[dependencies]
graphile_worker = { version = "0.13", default-features = false, features = [
  "runtime-tokio",
  "tls-rustls",
  "driver-sqlx",
] }
```

To use SQLx with async-std:

```toml
[dependencies]
graphile_worker = { version = "0.13", default-features = false, features = [
  "runtime-async-std",
  "tls-rustls",
  "driver-sqlx",
] }
```

To use the tokio-postgres driver:

```toml
[dependencies]
graphile_worker = { version = "0.13", default-features = false, features = [
  "driver-tokio-postgres",
] }
```

`driver-tokio-postgres` enables `runtime-tokio` for Graphile Worker RS. Add a
TLS feature only when the rest of your database stack needs one from this crate.

## SQLx Driver Executor Arguments

With `driver-sqlx`, the SQL helpers accept SQLx executors used by the tests:

- a pool
- an acquired connection
- a transaction

That means direct SQL helpers can participate in a caller-owned transaction. In
the tested path, a job added inside a SQLx transaction disappears after the
caller rolls the transaction back.

```rust,ignore
use graphile_worker::sql::add_job::single::add_job;
use graphile_worker::{JobSpec, Schema};
use serde_json::json;

let mut tx = pool.begin().await?;

add_job(
    &mut tx,
    &Schema::default(),
    "send_email",
    json!({ "user_id": 42 }),
    JobSpec::default(),
    false,
)
.await?;

tx.commit().await?;
```

The lower-level `DbExecutorArg` path is also tested for `execute`, `fetch_all`,
and `fetch_one` with SQLx pool, connection, and transaction executors.

## tokio-postgres Driver Executor Arguments

With `driver-tokio-postgres`, the SQL helpers accept tokio-postgres and
deadpool-postgres executor arguments used by the tests:

- `tokio_postgres::Client`
- `tokio_postgres::Transaction`
- `deadpool_postgres::Pool`
- a `deadpool_postgres` client checked out from the pool

```rust,ignore
use graphile_worker::sql::add_job::single::add_job;
use graphile_worker::{JobSpec, Schema};
use serde_json::json;
use tokio_postgres::NoTls;

let (client, connection) =
    tokio_postgres::connect("postgres://postgres:postgres@localhost/postgres", NoTls).await?;

tokio::spawn(async move {
    let _ = connection.await;
});

add_job(
    &client,
    &Schema::default(),
    "send_email",
    json!({ "user_id": 42 }),
    JobSpec::default(),
    false,
)
.await?;
```

The tested tokio-postgres path also verifies that jobs added through a
tokio-postgres transaction participate in the caller transaction and are not
persisted after rollback.

## Running the Matrix Locally

The project `justfile` defines targeted runtime and driver checks. If
`DATABASE_URL` is not set, the runtime target delegates to the Docker-backed
variant.

```bash
just test-runtime runtime-tokio driver-sqlx
just test-runtime runtime-async-std driver-sqlx
just test-runtime runtime-tokio driver-tokio-postgres
```

The combined Docker matrix runs the same supported combinations against a local
PostgreSQL container:

```bash
just test-docker-all-matrices
```
