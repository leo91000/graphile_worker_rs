use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    AsyncStd(async_std::task::JoinHandle<Result<T, JoinError>>),
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
            JoinHandleInner::AsyncStd(handle) => Pin::new(handle).poll(cx),
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
    let future = async move {
        match futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(future)).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err(JoinError::Aborted),
            Err(payload) => Err(JoinError::Failed(panic_payload_to_string(payload))),
        }
    };

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
    if deadline <= Instant::now() {
        return Err(TimeoutError);
    }

    let future = futures::FutureExt::fuse(future);
    let timeout = futures::FutureExt::fuse(sleep_until(deadline));
    futures::pin_mut!(future, timeout);

    futures::select_biased! {
        _ = timeout => Err(TimeoutError),
        result = future => Ok(result),
    }
}

#[cfg(feature = "runtime-async-std")]
fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    "task panicked".to_string()
}

pub fn interval(duration: Duration) -> Interval {
    assert!(duration > Duration::ZERO, "`period` must be non-zero.");

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
    state: AtomicUsize,
    event: Event,
}

const NOTIFY_COUNTER_BITS: usize = (usize::BITS as usize - 1) / 3;
const NOTIFY_COUNTER_MASK: usize = (1 << NOTIFY_COUNTER_BITS) - 1;
const NOTIFY_PERMIT: usize = 1;
const NOTIFY_WAITERS_SHIFT: usize = 1;
const NOTIFY_PENDING_SHIFT: usize = NOTIFY_WAITERS_SHIFT + NOTIFY_COUNTER_BITS;
const NOTIFY_BROADCAST_SHIFT: usize = NOTIFY_PENDING_SHIFT + NOTIFY_COUNTER_BITS;
const NOTIFY_WAITER: usize = 1 << NOTIFY_WAITERS_SHIFT;
const NOTIFY_PENDING: usize = 1 << NOTIFY_PENDING_SHIFT;

impl Notify {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify_one(&self) {
        if self.mark_one_notified() {
            self.event.notify_additional(1);
        }
    }

    pub fn notify_waiters(&self) {
        self.mark_all_notified();
        self.event.notify(usize::MAX);
    }

    pub fn notified(&self) -> Notified<'_> {
        Notified {
            notify: self,
            broadcast: notify_broadcast(self.state.load(Ordering::Acquire)),
            waiter: None,
            listener: None,
        }
    }

    fn mark_one_notified(&self) -> bool {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let waiters = notify_waiters(state);
            let pending = notify_pending(state);

            let new_state = if waiters > pending {
                state + NOTIFY_PENDING
            } else {
                state | NOTIFY_PERMIT
            };

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return waiters > pending;
            }
        }
    }

    fn mark_all_notified(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let broadcast = notify_broadcast(state);
            let new_broadcast = (broadcast + 1) & NOTIFY_COUNTER_MASK;
            let new_state = notify_with_waiters(
                notify_with_pending(notify_with_broadcast(state, new_broadcast), 0),
                0,
            );

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    fn register_waiter(&self, broadcast: usize) -> Option<EventListener> {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if notify_broadcast(state) != broadcast {
                return None;
            }

            if notify_has_permit(state) {
                let new_state = state & !NOTIFY_PERMIT;
                if self
                    .state
                    .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return None;
                }
                continue;
            }

            let waiters = notify_waiters(state);
            assert!(waiters < NOTIFY_COUNTER_MASK, "too many notify waiters");

            let listener = self.event.listen();
            if self
                .state
                .compare_exchange_weak(
                    state,
                    state + NOTIFY_WAITER,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Some(listener);
            }
        }
    }
}

struct NotifyWaiter<'a> {
    notify: &'a Notify,
    broadcast: usize,
    active: bool,
}

impl NotifyWaiter<'_> {
    fn try_complete(&mut self) -> bool {
        loop {
            let state = self.notify.state.load(Ordering::Acquire);
            let current_broadcast = notify_broadcast(state);
            let pending = notify_pending(state);

            if current_broadcast != self.broadcast {
                self.active = false;
                return true;
            }

            if pending == 0 {
                return false;
            }

            let waiters = notify_waiters(state);
            if waiters == 0 {
                self.active = false;
                return true;
            }

            let new_pending = pending - 1;
            let new_state =
                notify_with_pending(state - NOTIFY_WAITER, new_pending.min(waiters - 1));

            if self
                .notify
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.active = false;
                return true;
            }
        }
    }
}

impl Drop for NotifyWaiter<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let notify_another = loop {
            let state = self.notify.state.load(Ordering::Acquire);
            if notify_broadcast(state) != self.broadcast {
                break false;
            }

            let waiters = notify_waiters(state);
            if waiters == 0 {
                break false;
            }

            let pending = notify_pending(state);
            let new_waiters = waiters - 1;
            let new_pending = pending.min(new_waiters);
            let new_state = notify_with_pending(state - NOTIFY_WAITER, new_pending);

            if self
                .notify
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break pending > 0 && new_waiters > 0;
            }
        };

        if notify_another {
            self.notify.event.notify_additional(1);
        }
    }
}

pub struct Notified<'a> {
    notify: &'a Notify,
    broadcast: usize,
    waiter: Option<NotifyWaiter<'a>>,
    listener: Option<EventListener>,
}

impl Future for Notified<'_> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        loop {
            if this.waiter.is_none() {
                let listener = match this.notify.register_waiter(this.broadcast) {
                    Some(listener) => listener,
                    None => return Poll::Ready(()),
                };

                this.waiter = Some(NotifyWaiter {
                    notify: this.notify,
                    broadcast: this.broadcast,
                    active: true,
                });
                this.listener = Some(listener);
            }

            if this.waiter.as_mut().expect("notify waiter").try_complete() {
                this.waiter = None;
                this.listener = None;
                return Poll::Ready(());
            }

            let listener = this.listener.as_mut().expect("notify listener");
            if Pin::new(listener).poll(cx).is_pending() {
                return Poll::Pending;
            }

            if this.waiter.as_mut().expect("notify waiter").try_complete() {
                this.waiter = None;
                this.listener = None;
                return Poll::Ready(());
            }

            this.listener = Some(this.notify.event.listen());
        }
    }
}

fn notify_has_permit(state: usize) -> bool {
    state & NOTIFY_PERMIT != 0
}

fn notify_waiters(state: usize) -> usize {
    (state >> NOTIFY_WAITERS_SHIFT) & NOTIFY_COUNTER_MASK
}

fn notify_pending(state: usize) -> usize {
    (state >> NOTIFY_PENDING_SHIFT) & NOTIFY_COUNTER_MASK
}

fn notify_broadcast(state: usize) -> usize {
    (state >> NOTIFY_BROADCAST_SHIFT) & NOTIFY_COUNTER_MASK
}

fn notify_with_pending(state: usize, pending: usize) -> usize {
    let cleared = state & !(NOTIFY_COUNTER_MASK << NOTIFY_PENDING_SHIFT);
    cleared | (pending << NOTIFY_PENDING_SHIFT)
}

fn notify_with_waiters(state: usize, waiters: usize) -> usize {
    let cleared = state & !(NOTIFY_COUNTER_MASK << NOTIFY_WAITERS_SHIFT);
    cleared | (waiters << NOTIFY_WAITERS_SHIFT)
}

fn notify_with_broadcast(state: usize, broadcast: usize) -> usize {
    let cleared = state & !(NOTIFY_COUNTER_MASK << NOTIFY_BROADCAST_SHIFT);
    cleared | (broadcast << NOTIFY_BROADCAST_SHIFT)
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
    use std::time::{Duration, Instant};

    use futures::{executor::block_on, FutureExt};

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    use super::sleep;
    use super::{interval, timeout_at, JoinError, Notify};

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
    fn notify_waiters_wakes_futures_created_before_first_poll() {
        block_on(async {
            let notify = Notify::new();
            let pending = notify.notified().fuse();
            futures::pin_mut!(pending);

            notify.notify_waiters();

            assert!(matches!(futures::poll!(pending.as_mut()), Poll::Ready(())));
        });
    }

    #[test]
    fn notify_one_after_notify_waiters_stores_permit() {
        block_on(async {
            let notify = Notify::new();
            let first = notify.notified().fuse();
            let second = notify.notified().fuse();
            futures::pin_mut!(first, second);

            assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
            assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));

            notify.notify_waiters();
            notify.notify_one();

            assert!(matches!(futures::poll!(first.as_mut()), Poll::Ready(())));
            assert!(matches!(futures::poll!(second.as_mut()), Poll::Ready(())));

            let permit = notify.notified().fuse();
            futures::pin_mut!(permit);
            assert!(matches!(futures::poll!(permit.as_mut()), Poll::Ready(())));
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

    #[test]
    fn timeout_at_prefers_expired_deadline_over_ready_future() {
        block_on(async {
            let result = timeout_at(Instant::now() - Duration::from_millis(1), async { 42 }).await;

            assert!(result.is_err());
        });
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    fn tokio_spawn_completes_aborts_and_reports_panics() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        runtime.block_on(async {
            assert_eq!(super::spawn(async { 42 }).await.unwrap(), 42);

            let handle = super::spawn(async {
                sleep(Duration::from_secs(60)).await;
                7
            });
            handle.abort_handle().abort();
            assert!(matches!(handle.await, Err(JoinError::Aborted)));

            let result = super::spawn(async {
                panic!("spawn panic");
            })
            .await;

            match result {
                Err(JoinError::Failed(message)) => assert!(message.contains("spawn panic")),
                other => panic!("expected failed join error, got {other:?}"),
            }
        });
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    fn timeout_at_returns_ready_future_before_deadline() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        runtime.block_on(async {
            let result = timeout_at(Instant::now() + Duration::from_secs(60), async { 42 }).await;
            assert_eq!(result.unwrap(), 42);
        });
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    fn timeout_at_times_out_pending_future() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        runtime.block_on(async {
            let result = timeout_at(
                Instant::now() + Duration::from_millis(10),
                futures::future::pending::<()>(),
            )
            .await;
            assert!(result.is_err());
        });
    }

    #[test]
    #[should_panic(expected = "`period` must be non-zero.")]
    fn interval_rejects_zero_duration() {
        let _ = interval(Duration::ZERO);
    }

    #[cfg(feature = "runtime-async-std")]
    #[test]
    fn async_std_spawn_maps_task_panics_to_join_error() {
        block_on(async {
            let result = super::spawn(async {
                panic!("boom");
            })
            .await;

            match result {
                Err(super::JoinError::Failed(message)) => assert!(message.contains("boom")),
                other => panic!("expected failed join error, got {other:?}"),
            }
        });
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    #[should_panic(expected = "Tokio runtime")]
    fn tokio_only_spawn_requires_tokio_runtime() {
        drop(super::spawn(async {}));
    }

    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    #[test]
    #[should_panic(expected = "Tokio runtime")]
    fn tokio_only_sleep_requires_tokio_runtime() {
        drop(sleep(Duration::ZERO));
    }
}
