use std::time::Duration;

use crate::interval;

#[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
use crate::sleep;

#[test]
#[should_panic(expected = "`period` must be non-zero.")]
fn interval_rejects_zero_duration() {
    let _ = interval(Duration::ZERO);
}

#[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
#[test]
#[should_panic(expected = "Tokio runtime")]
fn tokio_only_sleep_requires_tokio_runtime() {
    drop(sleep(Duration::ZERO));
}
