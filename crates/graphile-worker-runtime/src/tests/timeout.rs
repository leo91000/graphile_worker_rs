use std::time::{Duration, Instant};

use crate::timeout_at;
use futures::executor::block_on;

#[test]
fn timeout_at_prefers_expired_deadline_over_ready_future() {
    block_on(async {
        let result = timeout_at(Instant::now() - Duration::from_millis(1), async { 42 }).await;

        assert!(result.is_err());
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
            Instant::now() - Duration::from_millis(1),
            futures::future::pending::<()>(),
        )
        .await;
        assert!(result.is_err());
    });
}
