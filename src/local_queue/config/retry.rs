use std::time::Duration;

use rand::RngExt;

pub(in crate::local_queue) struct RetryOptions {
    pub(in crate::local_queue) max_attempts: u32,
    pub(in crate::local_queue) min_delay_ms: u64,
    pub(in crate::local_queue) max_delay_ms: u64,
    pub(in crate::local_queue) multiplier: f64,
}

pub(in crate::local_queue) const RETURN_JOBS_RETRY_OPTIONS: RetryOptions = RetryOptions {
    max_attempts: 20,
    min_delay_ms: 200,
    max_delay_ms: 30_000,
    multiplier: 1.5,
};

pub(in crate::local_queue) fn calculate_retry_delay(
    attempt: u32,
    options: &RetryOptions,
) -> Duration {
    let base = options.min_delay_ms as f64 * options.multiplier.powi(attempt as i32);
    let capped = base.min(options.max_delay_ms as f64);
    let jittered = capped * (0.5 + rand::rng().random::<f64>() * 0.5);
    Duration::from_millis(jittered as u64)
}
