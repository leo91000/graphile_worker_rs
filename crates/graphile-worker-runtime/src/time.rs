use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Debug, Error)]
#[error("operation timed out")]
pub struct TimeoutError;

pub fn sleep(duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    #[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
    {
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| crate::missing_runtime());
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
        crate::missing_runtime()
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
