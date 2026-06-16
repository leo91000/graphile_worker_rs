use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use graphile_worker_runtime::Notify;
use graphile_worker_shutdown_signal::{shutdown_signal as os_shutdown_signal, ShutdownSignal};

use super::WorkerOptions;

/// Creates a shutdown signal that can be triggered manually via the returned notifier.
pub(super) fn manual_shutdown_signal_pair() -> (ShutdownSignal, Arc<Notify>) {
    let notify = Arc::new(Notify::new());
    let notify_for_signal = notify.clone();
    let signal = async move {
        notify_for_signal.notified().await;
    }
    .boxed()
    .shared();

    (signal, notify)
}

/// Resolves as soon as either of the provided shutdown signals completes.
pub(super) fn combine_shutdown_signals(
    left: ShutdownSignal,
    right: ShutdownSignal,
) -> ShutdownSignal {
    async move {
        let left = left.fuse();
        let right = right.fuse();
        futures::pin_mut!(left, right);
        futures::select_biased! {
            _ = left => (),
            _ = right => (),
        };
    }
    .boxed()
    .shared()
}

pub(super) fn configured_shutdown_signal(
    manual_signal: ShutdownSignal,
    config: &crate::WorkerShutdownConfig,
) -> ShutdownSignal {
    let mut signal = manual_signal;

    if let Some(custom_signal) = config.shutdown_signal.clone() {
        signal = combine_shutdown_signals(signal, custom_signal);
    }

    if config.listen_os_shutdown_signals {
        signal = combine_shutdown_signals(signal, os_shutdown_signal());
    }

    signal
}

impl WorkerOptions {
    fn update_shutdown_config(
        mut self,
        update: impl FnOnce(&mut crate::WorkerShutdownConfig),
    ) -> Self {
        let mut config = self.worker_shutdown_config.unwrap_or_default();
        update(&mut config);
        self.worker_shutdown_config = Some(config);
        self
    }

    /// Configures worker shutdown behavior.
    pub fn worker_shutdown(mut self, config: crate::WorkerShutdownConfig) -> Self {
        self.worker_shutdown_config = Some(config);
        self
    }

    /// Controls whether the worker installs OS-level shutdown signal handlers.
    ///
    /// By default Graphile Worker listens to signals like SIGINT/SIGTERM to
    /// trigger a graceful shutdown. Embedding applications that already manage
    /// signal handling can disable this behavior by setting the value to `false`.
    ///
    /// # Arguments
    /// * `value` - `true` to install the default OS signal listeners, `false` to skip them
    pub fn listen_os_shutdown_signals(self, value: bool) -> Self {
        self.update_shutdown_config(|config| {
            config.listen_os_shutdown_signals = value;
        })
    }

    /// Sets a custom application shutdown signal.
    ///
    /// The future should complete when the host application requests shutdown.
    /// The worker still owns graceful draining after the signal is received.
    pub fn shutdown_signal(
        self,
        signal: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> Self {
        self.update_shutdown_config(|config| {
            config.shutdown_signal = Some(crate::shutdown::future_to_shutdown_signal(signal));
        })
    }

    /// Sets how long in-flight jobs may continue after a shutdown signal.
    pub fn shutdown_grace_period(self, period: Duration) -> Self {
        self.update_shutdown_config(|config| {
            config.grace_period = period;
        })
    }

    /// Sets the delay before shutdown-aborted jobs are retried.
    pub fn shutdown_interrupted_job_retry_delay(self, delay: Duration) -> Self {
        self.update_shutdown_config(|config| {
            config.interrupted_job_retry_delay = delay;
        })
    }
}
