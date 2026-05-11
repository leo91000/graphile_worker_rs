use std::pin::Pin;

use futures::{future::Shared, FutureExt};
use std::future::Future;
use tracing::info;

#[cfg(unix)]
use async_signal::{Signal, Signals};

#[cfg(unix)]
use futures::StreamExt;

#[cfg(windows)]
use event_listener::Event;

#[cfg(windows)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

#[cfg(windows)]
use windows_sys::Win32::System::Console::{
    SetConsoleCtrlHandler, CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
};

#[cfg(windows)]
use windows_sys::core::BOOL;

#[cfg(not(any(unix, windows)))]
compile_error!("Your OS does not support shutdown signal ! Are you targeting wasm ?");

#[cfg(windows)]
static WINDOWS_SHUTDOWN_EVENT: Event = Event::new();
#[cfg(windows)]
static WINDOWS_SHUTDOWN_RECEIVED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static WINDOWS_HANDLER_REGISTERED: OnceLock<()> = OnceLock::new();

#[cfg(windows)]
const WINDOWS_SIGNAL_HANDLED: BOOL = 1;
#[cfg(windows)]
const WINDOWS_SIGNAL_UNHANDLED: BOOL = 0;

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        CTRL_C_EVENT | CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => {
            WINDOWS_SHUTDOWN_RECEIVED.store(true, Ordering::Release);
            WINDOWS_SHUTDOWN_EVENT.notify(usize::MAX);
            WINDOWS_SIGNAL_HANDLED
        }
        _ => WINDOWS_SIGNAL_UNHANDLED,
    }
}

#[cfg(windows)]
fn register_windows_shutdown_handler() {
    WINDOWS_HANDLER_REGISTERED.get_or_init(|| {
        let result =
            unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), WINDOWS_SIGNAL_HANDLED) };
        if result == WINDOWS_SIGNAL_UNHANDLED {
            panic!("Failed to listen to windows shutdown signal");
        }
    });
}

#[cfg(windows)]
async fn raw_shutdown_signal() {
    register_windows_shutdown_handler();
    if WINDOWS_SHUTDOWN_RECEIVED.load(Ordering::Acquire) {
        return;
    }
    let listener = WINDOWS_SHUTDOWN_EVENT.listen();
    if WINDOWS_SHUTDOWN_RECEIVED.load(Ordering::Acquire) {
        return;
    }
    listener.await;
}

#[cfg(unix)]
async fn raw_shutdown_signal() {
    let mut signals = Signals::new([
        Signal::Usr2,
        Signal::Int,
        Signal::Pipe,
        Signal::Term,
        Signal::Hup,
    ])
    .expect("Failed to listen to unix shutdown signal");
    let _ = signals.next().await;
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
