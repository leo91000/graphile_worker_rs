// Retry policy adapted from graphile/worker src/lib.ts; see LICENSE.md.
use std::future::Future;
use std::num::NonZeroU32;
use std::time::Duration;

use crate::DbError;

/// Opt-in retries for pooled statements whose failure is known to precede commit.
///
/// Uses Graphile Worker's exponential backoff: 50ms for serialization/deadlock,
/// 3s for unavailable connections, multiplied by 1.5 per retry, capped at 5s
/// and 120s respectively, then jittered by 0.5–1.5. The default is 100 total
/// attempts. Transactions and LISTEN
/// are excluded; callers must retry an entire transaction themselves.
#[derive(Clone, Copy, Debug)]
pub struct SqlRetryPolicy {
    max_attempts: NonZeroU32,
}

impl Default for SqlRetryPolicy {
    /// Uses the upstream limit of 100 total attempts.
    fn default() -> Self {
        Self::new(NonZeroU32::new(100).unwrap())
    }
}

impl SqlRetryPolicy {
    /// Sets the total attempt limit, including the first execution.
    pub fn new(max_attempts: NonZeroU32) -> Self {
        Self { max_attempts }
    }

    /// Returns the total attempt limit; one disables retries.
    pub fn max_attempts(&self) -> NonZeroU32 {
        self.max_attempts
    }

    /// Replays only explicit aborted statements or pre-execution connection faults.
    pub(crate) async fn run<T, F, Fut>(&self, mut operation: F) -> Result<T, DbError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, DbError>>,
    {
        for attempt in 1..self.max_attempts.get() {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(error) => match Self::backoff(&error, attempt - 1, rand::random()) {
                    Some(delay) => graphile_worker_runtime::sleep(delay).await,
                    None => return Err(error),
                },
            }
        }
        operation().await
    }

    /// Never retries ambiguous execution-time I/O failures or other SQLSTATEs.
    fn backoff(error: &DbError, previous_attempts: u32, jitter: f64) -> Option<Duration> {
        let (minimum, maximum) = match error.code() {
            Some("40001" | "40P01") => (50.0, 5_000.0),
            Some("57P03") => (3000.0, 120_000.0),
            Some("EHOSTUNREACH" | "ETIMEDOUT") if error.is_connection_failure() => {
                (3000.0, 120_000.0)
            }
            _ => return None,
        };
        Some(Duration::from_secs_f64(
            (minimum * 1.5_f64.powf(f64::from(previous_attempts))).min(maximum) * (0.5 + jitter)
                / 1000.0,
        ))
    }
}

#[cfg(all(test, feature = "runtime-tokio"))]
mod tests;

#[cfg(all(test, feature = "runtime-async-std"))]
mod async_std_tests {
    /// A retry sleep must work when no Tokio runtime is present.
    #[test]
    fn retry_uses_async_std_without_a_tokio_runtime() {
        async_std::task::block_on(async {
            assert!(tokio::runtime::Handle::try_current().is_err());
            let mut calls = 0;
            let result = super::SqlRetryPolicy::default()
                .run(|| {
                    calls += 1;
                    let call = calls;
                    async move {
                        if call == 1 {
                            Err(crate::DbError::with_code("retry", "40001"))
                        } else {
                            Ok(42)
                        }
                    }
                })
                .await
                .unwrap();
            assert_eq!(result, 42);
            assert_eq!(calls, 2);
        });
    }
}
