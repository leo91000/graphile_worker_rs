use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::{Abortable, Aborted};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JoinError {
    #[error("task was aborted")]
    Aborted,
    #[error("task failed: {0}")]
    Failed(String),
}

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
        let handle =
            tokio::runtime::Handle::try_current().unwrap_or_else(|_| crate::missing_runtime());
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
        crate::missing_runtime()
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
