# Shutdown

Graphile Worker shuts down gracefully when its shutdown signal completes. By
default this includes OS shutdown signals, but applications that already own
process lifecycle management can provide their own future instead.

Shutdown behavior is configured with `WorkerShutdownConfig` or with the
matching convenience methods on `WorkerOptions`.

## Defaults

`WorkerShutdownConfig::default()` uses these values:

```rust,ignore
use graphile_worker::WorkerShutdownConfig;
use std::time::Duration;

let shutdown = WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(true)
    .grace_period(Duration::from_secs(5))
    .interrupted_job_retry_delay(Duration::from_secs(30));
```

The default configuration:

- listens for OS shutdown signals
- gives in-flight jobs 5 seconds to finish after shutdown starts
- retries shutdown-aborted jobs after 30 seconds
- has no custom application shutdown signal

## OS Signals

When `listen_os_shutdown_signals` is enabled, Graphile Worker installs the
platform shutdown listener provided by `graphile-worker-shutdown-signal`.

On Unix targets, the listener completes after one of these signals is received:

- `SIGUSR2`
- `SIGINT`
- `SIGPIPE`
- `SIGTERM`
- `SIGHUP`

On Windows targets, the listener handles these console control events:

- `CTRL_C_EVENT`
- `CTRL_CLOSE_EVENT`
- `CTRL_LOGOFF_EVENT`
- `CTRL_SHUTDOWN_EVENT`

This is enabled by default:

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .listen_os_shutdown_signals(true)
    // ... other configuration
    .init()
    .await?;
```

Disable OS signal listeners when the host application already installs its own
handlers:

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .listen_os_shutdown_signals(false)
    // ... other configuration
    .init()
    .await?;
```

## Custom Shutdown Signal

A custom shutdown signal is any `Future<Output = ()> + Send + 'static`. The
future should complete when the host application requests shutdown.

```rust,ignore
use graphile_worker::{WorkerOptions, WorkerShutdownConfig};

let shutdown = WorkerShutdownConfig::default()
    .listen_os_shutdown_signals(false)
    .shutdown_signal(async {
        wait_for_application_shutdown().await;
    });

let worker = WorkerOptions::default()
    .worker_shutdown(shutdown)
    // ... other configuration
    .init()
    .await?;

worker.run().await?;
```

You can also set the signal directly on `WorkerOptions`:

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .listen_os_shutdown_signals(false)
    .shutdown_signal(async {
        wait_for_application_shutdown().await;
    })
    // ... other configuration
    .init()
    .await?;
```

If both OS signal listening and a custom signal are configured, shutdown starts
when either signal completes.

## Grace Period

`grace_period` controls how long in-flight jobs may continue after a shutdown
signal is received.

```rust,ignore
use graphile_worker::WorkerShutdownConfig;
use std::time::Duration;

let shutdown = WorkerShutdownConfig::default()
    .grace_period(Duration::from_secs(20));
```

The equivalent `WorkerOptions` convenience method is:

```rust,ignore
use graphile_worker::WorkerOptions;
use std::time::Duration;

let worker = WorkerOptions::default()
    .shutdown_grace_period(Duration::from_secs(20))
    // ... other configuration
    .init()
    .await?;
```

During shutdown, Graphile Worker still owns graceful draining after the signal
is received.

## Interrupted Job Retry Delay

`interrupted_job_retry_delay` controls when a job aborted by shutdown becomes
eligible to run again.

```rust,ignore
use graphile_worker::WorkerShutdownConfig;
use std::time::Duration;

let shutdown = WorkerShutdownConfig::default()
    .interrupted_job_retry_delay(Duration::from_secs(60));
```

The equivalent `WorkerOptions` convenience method is:

```rust,ignore
use graphile_worker::WorkerOptions;
use std::time::Duration;

let worker = WorkerOptions::default()
    .shutdown_interrupted_job_retry_delay(Duration::from_secs(60))
    // ... other configuration
    .init()
    .await?;
```

Shutdown-aborted jobs use this delay instead of normal failure backoff, so
shutdown does not consume a retry attempt. This behavior applies to ordinary
graceful shutdown even when [worker recovery](../guides/recovery.md) is
disabled.
