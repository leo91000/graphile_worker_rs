use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use event_listener::Event;
use futures::future::{Abortable, Aborted};
use thiserror::Error;

#[cfg(all(not(feature = "runtime-tokio"), not(feature = "runtime-async-std")))]
compile_error!("Either runtime-tokio or runtime-async-std must be enabled");

pub use async_channel::{bounded as channel, Receiver, Sender};
pub use async_lock::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug, Error)]
pub enum JoinError {
    #[error("task was aborted")]
    Aborted,
    #[error("task failed: {0}")]
    Failed(String),
}

#[derive(Debug, Error)]
#[error("operation timed out")]
pub struct TimeoutError;

#[derive(Clone)]
pub struct AbortHandle(futures::future::AbortHandle);

impl AbortHandle {
    pub fn abort(&self) {
        self.0.abort();
    }
}

pub struct JoinHandle<T> {
    abort_handle: AbortHandle,
    inner: JoinHandleInner<T>,
}

enum JoinHandleInner<T> {
    #[cfg(feature = "runtime-tokio")]
    Tokio(tokio::task::JoinHandle<Result<T, Aborted>>),
    #[cfg(feature = "runtime-async-std")]
    AsyncStd(async_std::task::JoinHandle<Result<T, Aborted>>),
}

impl<T> JoinHandle<T> {
    pub fn abort_handle(&self) -> AbortHandle {
        self.abort_handle.clone()
    }
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.inner {
            #[cfg(feature = "runtime-tokio")]
            JoinHandleInner::Tokio(handle) => {
                Pin::new(handle).poll(cx).map(|result| match result {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(_)) => Err(JoinError::Aborted),
                    Err(error) => Err(JoinError::Failed(error.to_string())),
                })
            }
            #[cfg(feature = "runtime-async-std")]
            JoinHandleInner::AsyncStd(handle) => {
                Pin::new(handle).poll(cx).map(|result| match result {
                    Ok(value) => Ok(value),
                    Err(_) => Err(JoinError::Aborted),
                })
            }
        }
    }
}

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (abort_handle, abort_registration) = futures::future::AbortHandle::new_pair();
    let future = Abortable::new(future, abort_registration);
    let abort_handle = AbortHandle(abort_handle);

    #[cfg(feature = "runtime-tokio")]
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return JoinHandle {
            abort_handle,
            inner: JoinHandleInner::Tokio(handle.spawn(future)),
        };
    }

    #[cfg(feature = "runtime-async-std")]
    {
        async_std_spawn(future, abort_handle)
    }

    #[cfg(not(feature = "runtime-async-std"))]
    {
        missing_runtime()
    }
}

#[cfg(feature = "runtime-async-std")]
fn async_std_spawn<F, T>(future: F, abort_handle: AbortHandle) -> JoinHandle<T>
where
    F: Future<Output = Result<T, Aborted>> + Send + 'static,
    T: Send + 'static,
{
    JoinHandle {
        abort_handle,
        inner: JoinHandleInner::AsyncStd(async_std::task::spawn(future)),
    }
}

pub fn sleep(duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    #[cfg(feature = "runtime-tokio")]
    if tokio::runtime::Handle::try_current().is_ok() {
        return Box::pin(tokio::time::sleep(duration));
    }

    #[cfg(feature = "runtime-async-std")]
    {
        Box::pin(async_std::task::sleep(duration))
    }

    #[cfg(not(feature = "runtime-async-std"))]
    {
        missing_runtime()
    }
}

pub fn sleep_until(deadline: Instant) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let now = Instant::now();
        if deadline > now {
            sleep(deadline - now).await;
        }
    })
}

pub async fn timeout_at<F>(deadline: Instant, future: F) -> Result<F::Output, TimeoutError>
where
    F: Future,
{
    let future = futures::FutureExt::fuse(future);
    let timeout = futures::FutureExt::fuse(sleep_until(deadline));
    futures::pin_mut!(future, timeout);

    futures::select_biased! {
        result = future => Ok(result),
        _ = timeout => Err(TimeoutError),
    }
}

pub fn interval(duration: Duration) -> Interval {
    Interval {
        duration,
        next: Instant::now(),
    }
}

pub struct Interval {
    duration: Duration,
    next: Instant,
}

impl Interval {
    pub async fn tick(&mut self) {
        let now = Instant::now();
        if self.next > now {
            sleep_until(self.next).await;
        }
        self.next += self.duration;
        let now = Instant::now();
        if self.next < now {
            self.next = now + self.duration;
        }
    }
}

#[derive(Default)]
pub struct Notify {
    permits: AtomicUsize,
    event: Event,
}

impl Notify {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify_one(&self) {
        self.add_permit();
        self.event.notify(1);
    }

    pub fn notify_waiters(&self) {
        self.add_permit();
        self.event.notify(usize::MAX);
    }

    pub async fn notified(&self) {
        if self.take_permit() {
            return;
        }

        loop {
            let listener = self.event.listen();
            if self.take_permit() {
                return;
            }
            listener.await;
            if self.take_permit() {
                return;
            }
        }
    }

    fn take_permit(&self) -> bool {
        self.permits
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |permits| {
                (permits > 0).then(|| permits - 1)
            })
            .is_ok()
    }

    fn add_permit(&self) {
        let _ = self
            .permits
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |permits| {
                Some(permits.saturating_add(1))
            });
    }
}

#[cfg(not(feature = "runtime-async-std"))]
fn missing_runtime<T>() -> T {
    if cfg!(feature = "runtime-tokio") {
        panic!("this functionality requires a Tokio runtime")
    }

    panic!("Either runtime-tokio or runtime-async-std must be enabled")
}
