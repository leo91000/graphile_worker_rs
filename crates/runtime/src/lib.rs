use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex as StdMutex, MutexGuard};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use event_listener::{Event, EventListener};
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

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    {
        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| missing_runtime());
        JoinHandle {
            abort_handle,
            inner: JoinHandleInner::Tokio(handle.spawn(future)),
        }
    }

    #[cfg(all(feature = "runtime-tokio", feature = "runtime-async-std"))]
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

    #[cfg(not(any(feature = "runtime-tokio", feature = "runtime-async-std")))]
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
    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    {
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| missing_runtime());
        Box::pin(tokio::time::sleep(duration))
    }

    #[cfg(all(feature = "runtime-tokio", feature = "runtime-async-std"))]
    if tokio::runtime::Handle::try_current().is_ok() {
        return Box::pin(tokio::time::sleep(duration));
    }

    #[cfg(feature = "runtime-async-std")]
    {
        Box::pin(async_std::task::sleep(duration))
    }

    #[cfg(not(any(feature = "runtime-tokio", feature = "runtime-async-std")))]
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
    state: StdMutex<NotifyState>,
    event: Event,
}

#[derive(Default)]
struct NotifyState {
    permit: bool,
    waiters: usize,
    pending: usize,
    broadcast: usize,
}

impl Notify {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify_one(&self) {
        if self.mark_one_notified() {
            self.event.notify(1);
        }
    }

    pub fn notify_waiters(&self) {
        if self.mark_all_notified() {
            self.event.notify(usize::MAX);
        }
    }

    pub async fn notified(&self) {
        let (mut listener, broadcast) = match self.register_waiter() {
            Some(waiter) => waiter,
            None => return,
        };
        let mut waiter = NotifyWaiter {
            notify: self,
            active: true,
        };

        if waiter.try_complete(broadcast) {
            return;
        }

        loop {
            listener.await;

            if waiter.try_complete(broadcast) {
                return;
            }

            listener = self.event.listen();

            if waiter.try_complete(broadcast) {
                return;
            }
        }
    }

    fn mark_one_notified(&self) -> bool {
        let mut state = self.lock_state();
        if state.waiters > state.pending {
            state.pending += 1;
            return true;
        }

        state.permit = true;
        false
    }

    fn mark_all_notified(&self) -> bool {
        let mut state = self.lock_state();
        if state.waiters == 0 {
            return false;
        }

        state.pending = 0;
        state.broadcast = state.broadcast.wrapping_add(1);
        true
    }

    fn register_waiter(&self) -> Option<(EventListener, usize)> {
        let mut state = self.lock_state();
        if state.permit {
            state.permit = false;
            return None;
        }

        state.waiters += 1;
        Some((self.event.listen(), state.broadcast))
    }

    fn lock_state(&self) -> MutexGuard<'_, NotifyState> {
        self.state.lock().expect("notify state poisoned")
    }
}

struct NotifyWaiter<'a> {
    notify: &'a Notify,
    active: bool,
}

impl NotifyWaiter<'_> {
    fn try_complete(&mut self, broadcast: usize) -> bool {
        let mut state = self.notify.lock_state();
        if state.broadcast != broadcast {
            self.unregister(&mut state);
            return true;
        }

        if state.pending > 0 {
            state.pending -= 1;
            self.unregister(&mut state);
            return true;
        }

        false
    }

    fn unregister(&mut self, state: &mut NotifyState) {
        state.waiters -= 1;
        state.pending = state.pending.min(state.waiters);
        self.active = false;
    }
}

impl Drop for NotifyWaiter<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let mut state = self.notify.lock_state();
        self.unregister(&mut state);
    }
}

#[cfg(not(feature = "runtime-async-std"))]
fn missing_runtime<T>() -> T {
    if cfg!(feature = "runtime-tokio") {
        panic!("this functionality requires a Tokio runtime")
    }

    panic!("Either runtime-tokio or runtime-async-std must be enabled")
}

#[cfg(test)]
mod tests {
    use std::task::Poll;

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    use std::time::Duration;

    use futures::{executor::block_on, FutureExt};

    use super::Notify;
    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    use super::{sleep, spawn};

    #[test]
    fn notify_one_coalesces_pending_permits() {
        block_on(async {
            let notify = Notify::new();
            notify.notify_one();
            notify.notify_one();
            notify.notified().await;

            let pending = notify.notified().fuse();
            futures::pin_mut!(pending);

            assert!(matches!(futures::poll!(pending.as_mut()), Poll::Pending));
        });
    }

    #[test]
    fn notify_waiters_wakes_all_registered_waiters() {
        block_on(async {
            let notify = Notify::new();
            let first = notify.notified().fuse();
            let second = notify.notified().fuse();
            futures::pin_mut!(first, second);

            assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
            assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));

            notify.notify_waiters();

            assert!(matches!(futures::poll!(first.as_mut()), Poll::Ready(())));
            assert!(matches!(futures::poll!(second.as_mut()), Poll::Ready(())));
        });
    }

    #[test]
    fn notify_waiters_does_not_store_a_permit() {
        block_on(async {
            let notify = Notify::new();
            notify.notify_waiters();

            let pending = notify.notified().fuse();
            futures::pin_mut!(pending);

            assert!(matches!(futures::poll!(pending.as_mut()), Poll::Pending));
        });
    }

    #[test]
    fn notify_one_wakes_one_registered_waiter_per_call() {
        block_on(async {
            let notify = Notify::new();
            let first = notify.notified().fuse();
            let second = notify.notified().fuse();
            futures::pin_mut!(first, second);

            assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
            assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));

            notify.notify_one();

            let first_ready = matches!(futures::poll!(first.as_mut()), Poll::Ready(()));
            let second_ready = matches!(futures::poll!(second.as_mut()), Poll::Ready(()));
            assert_eq!(usize::from(first_ready) + usize::from(second_ready), 1);

            notify.notify_one();

            if !first_ready {
                assert!(matches!(futures::poll!(first.as_mut()), Poll::Ready(())));
            }
            if !second_ready {
                assert!(matches!(futures::poll!(second.as_mut()), Poll::Ready(())));
            }
        });
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    #[should_panic(expected = "Tokio runtime")]
    fn tokio_only_spawn_requires_tokio_runtime() {
        drop(spawn(async {}));
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    #[should_panic(expected = "Tokio runtime")]
    fn tokio_only_sleep_requires_tokio_runtime() {
        drop(sleep(Duration::ZERO));
    }
}
