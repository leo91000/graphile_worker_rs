use std::pin::Pin;

use cfg_if::cfg_if;
use futures::{future::Shared, FutureExt};
use std::future::Future;
use tokio::select;
use tracing::info;

cfg_if! {
    if #[cfg(windows)] {
        use tokio::signal::windows::*;

        async fn raw_shutdown_signal() {
            let mut  ctrl_c = ctrl_c().expect("Failed to attach Ctrl_C shutdown signal (windows)");
            let mut  ctrl_close = ctrl_close().expect("Failed to attach Ctrl_close shutdown signal (windows)");
            let mut  ctrl_shutdown = ctrl_shutdown().expect("Failed to attach Ctrl_shutdown shutdown signal (windows)");
            let mut  ctrl_logoff = ctrl_logoff().expect("Failed to attach Ctrl_logoff shutdown signal (windows)");
            select! {
                _ = ctrl_c.recv() => (),
                _ = ctrl_close.recv() => (),
                _ = ctrl_shutdown.recv() => (),
                _ = ctrl_logoff.recv() => (),
            }
        }
    } else if #[cfg(unix)] {
        use tokio::signal::unix::*;

        async fn unix_shutdown_signal(signal_kind: SignalKind) {
            let mut signal = signal(signal_kind).expect("Failed to listen to unix shutdown signal");
            signal.recv().await;
        }

        async fn raw_shutdown_signal() {
            select! {
                _ = unix_shutdown_signal(SignalKind::user_defined2()) => (),
                _ = unix_shutdown_signal(SignalKind::interrupt()) => (),
                _ = unix_shutdown_signal(SignalKind::pipe()) => (),
                _ = unix_shutdown_signal(SignalKind::terminate()) => (),
                _ = unix_shutdown_signal(SignalKind::hangup()) => (),
                // _ = unix_shutdown_signal(SignalKind::from_raw(std::os::raw::c_int::SIGABRT)) => (),
            };
        }
    } else {
        compile_error!("Your OS does not support shutdown signal ! Are you targeting wasm ?");
    }
}

pub type ShutdownSignal = Shared<Pin<Box<dyn Future<Output = ()> + Send>>>;

pub fn shutdown_signal() -> ShutdownSignal {
    async {
        raw_shutdown_signal().await;
        info!("Shutdown signal detected. Attempting graceful shutdown...");
    }
    .boxed()
    .shared()
}
