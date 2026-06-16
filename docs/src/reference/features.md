# Feature Flags

The `graphile_worker` crate uses feature flags to select the async runtime, TLS
implementation, PostgreSQL driver, and optional OpenTelemetry compatibility
version.

Default features enable the most common setup:

```toml
[dependencies]
graphile_worker = "0.13"
```

This is equivalent to enabling:

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  features = ["runtime-tokio", "tls-rustls", "driver-sqlx"],
}
```

## Defaults

The default feature set is:

- `runtime-tokio`
- `tls-rustls`
- `driver-sqlx`

Use `default-features = false` when you want to choose a different runtime,
driver, or TLS backend.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["runtime-async-std", "tls-rustls", "driver-sqlx"],
}
```

## Runtime Features

Runtime features select the async runtime used by the root crate and its
internal crates.

| Feature | Purpose |
| --- | --- |
| `runtime-tokio` | Enables Tokio runtime support. This is enabled by default. |
| `runtime-async-std` | Enables async-std runtime support. |

The `driver-tokio-postgres` feature also enables `runtime-tokio`, so that driver
is a Tokio-only combination in the root crate feature graph.

## TLS Features

TLS features select the TLS implementation forwarded to database-related
crates and, when SQLx is enabled, to SQLx.

| Feature | Purpose |
| --- | --- |
| `tls-rustls` | Enables rustls TLS support. This is enabled by default. |
| `tls-native-tls` | Enables native-tls support. |

The root feature list does not force exactly one TLS backend. Pick the one you
intend to use instead of enabling both accidentally.

## Database Driver Features

Driver features select the PostgreSQL access layer used by database-related
crates.

| Feature | Purpose |
| --- | --- |
| `driver-sqlx` | Enables SQLx support and the optional `sqlx` dependency. This is enabled by default. |
| `driver-tokio-postgres` | Enables tokio-postgres support and also enables `runtime-tokio`. |

SQLx is the default driver. The tokio-postgres driver is available as an
alternative Tokio-based driver.

## OpenTelemetry Features

OpenTelemetry support is versioned so downstream applications can choose the
compatibility line they use.

| Feature | Dependencies enabled |
| --- | --- |
| `opentelemetry_0_30` | `opentelemetry` 0.30 and matching `tracing-opentelemetry` support |
| `opentelemetry_0_31` | `opentelemetry` 0.31 and matching `tracing-opentelemetry` support |
| `opentelemetry_0_32` | `opentelemetry` 0.32 and matching `tracing-opentelemetry` support |

These features are not enabled by default. Enable the one that matches the
OpenTelemetry version used by the rest of your application.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  features = ["opentelemetry_0_32"],
}
```

## Common Combinations

### Default Tokio, rustls, SQLx

Use the default dependency declaration when you want Tokio, rustls, and SQLx.

```toml
[dependencies]
graphile_worker = "0.13"
```

### Tokio, native-tls, SQLx

Disable defaults and select the TLS backend explicitly.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["runtime-tokio", "tls-native-tls", "driver-sqlx"],
}
```

### async-std, rustls, SQLx

Use the async-std runtime with the SQLx driver.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["runtime-async-std", "tls-rustls", "driver-sqlx"],
}
```

### Tokio, tokio-postgres

The tokio-postgres driver enables `runtime-tokio` through the feature graph.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["driver-tokio-postgres"],
}
```

If your connection setup needs TLS, also select the TLS feature you intend to
use:

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["driver-tokio-postgres", "tls-rustls"],
}
```

### Tokio, SQLx, OpenTelemetry 0.32

OpenTelemetry features can be added to the default feature set.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  features = ["opentelemetry_0_32"],
}
```

## Feature Matrix Used by Local Checks

The repository's runtime matrix checks the following combinations:

```sh
just test-docker-all-matrices
```

That command covers:

| Runtime | Driver | TLS |
| --- | --- | --- |
| `runtime-tokio` | `driver-sqlx` | `tls-rustls` |
| `runtime-async-std` | `driver-sqlx` | `tls-rustls` |
| `runtime-tokio` | `driver-tokio-postgres` | not selected by the test recipe |

The `test-runtime` recipe rejects `driver-tokio-postgres` with
`runtime-async-std`, matching the feature relationship where
`driver-tokio-postgres` enables Tokio.

## Cautions

Disable defaults when replacing a default feature. For example, adding
`tls-native-tls` without `default-features = false` leaves `tls-rustls` enabled
too.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["runtime-tokio", "tls-native-tls", "driver-sqlx"],
}
```

Choose one database driver unless you have checked that your application really
needs both. The default driver is SQLx, so enabling `driver-tokio-postgres`
without disabling defaults enables both drivers.

```toml
[dependencies]
graphile_worker = {
  version = "0.13",
  default-features = false,
  features = ["driver-tokio-postgres"],
}
```

Use an OpenTelemetry feature only when your dependency graph uses that matching
OpenTelemetry line. The root crate exposes separate compatibility features for
0.30, 0.31, and 0.32.

Illustrative Rust examples in this documentation may require additional setup,
such as task handlers, worker options, and a database URL. See the
[quick start](../getting-started/quick-start.md) for a complete first worker.

```rust,ignore
// Feature selection happens in Cargo.toml, not in Rust source.
use graphile_worker::WorkerOptions;
```
