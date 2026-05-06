use std::pin::Pin;

use async_signal::{Signal, Signals};
use cfg_if::cfg_if;
use futures::{future::Shared, FutureExt, StreamExt};
use std::future::Future;
use tracing::info;

cfg_if! {
    if #[cfg(windows)] {
        async fn raw_shutdown_signal() {
            let mut signals = Signals::new([Signal::Int]).expect("Failed to listen to windows shutdown signal");
            let _ = signals.next().await;
        }
    } else if #[cfg(unix)] {
        async fn raw_shutdown_signal() {
            let mut signals = Signals::new([
                Signal::Usr2,
                Signal::Int,
                Signal::Pipe,
                Signal::Term,
                Signal::Hup,
            ]).expect("Failed to listen to unix shutdown signal");
            let _ = signals.next().await;
        }
    } else {
        compile_error!("Your OS does not support shutdown signal ! Are you targeting wasm ?");
    }
}

/// A shareable future that completes when a shutdown signal is received.
///
/// This type is a future that can be cloned and shared between multiple
/// consumers who need to be notified when a shutdown signal is received.
/// When awaited, it will only complete when the process receives a shutdown
/// signal (like Ctrl+C, SIGTERM, etc.).
pub type ShutdownSignal = Shared<Pin<Box<dyn Future<Output = ()> + Send>>>;

/// Creates a new shareable shutdown signal detector.
///
/// This function returns a `ShutdownSignal` that can be cloned and shared
/// across different components. Each clone of the signal will complete
/// when the process receives a shutdown signal from the operating system,
/// such as Ctrl+C (SIGINT), SIGTERM, etc.
///
/// # Returns
///
/// A `ShutdownSignal` that can be cloned and awaited
///
/// # Examples
///
/// ```ignore
/// use graphile_worker_shutdown_signal::shutdown_signal;
/// use tokio::select;
/// use tokio::time::{sleep, Duration};
///
/// async fn some_long_running_task() {
///     sleep(Duration::from_secs(60)).await;
/// }
///
/// async fn example() {
///     // Create a shutdown signal
///     let signal = shutdown_signal();
///     
///     // Use in select to implement graceful shutdown
///     select! {
///         _ = signal => {
///             println!("Shutting down gracefully...");
///             // Cleanup resources
///         }
///         _ = some_long_running_task() => {
///             println!("Task completed!");
///         }
///     }
/// }
/// ```
pub fn shutdown_signal() -> ShutdownSignal {
    async {
        raw_shutdown_signal().await;
        info!("Shutdown signal detected. Attempting graceful shutdown...");
    }
    .boxed()
    .shared()
}
