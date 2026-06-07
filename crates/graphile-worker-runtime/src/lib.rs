#[cfg(all(not(feature = "runtime-tokio"), not(feature = "runtime-async-std")))]
compile_error!("Either runtime-tokio or runtime-async-std must be enabled");

mod notify;
mod task;
mod time;

#[cfg(test)]
mod tests;

pub use async_channel::{bounded as channel, Receiver, Sender};
pub use async_lock::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
pub use notify::{Notified, Notify};
pub use task::{spawn, AbortHandle, JoinError, JoinHandle};
pub use time::{interval, sleep, sleep_until, timeout_at, Interval, TimeoutError};

#[cfg(not(feature = "runtime-async-std"))]
pub(crate) fn missing_runtime<T>() -> T {
    if cfg!(feature = "runtime-tokio") {
        panic!("this functionality requires a Tokio runtime")
    }

    panic!("Either runtime-tokio or runtime-async-std must be enabled")
}
