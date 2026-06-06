use std::time::Duration;

use super::WorkerOptions;

impl WorkerOptions {
    /// Sets the delay before flushing batched job completions.
    ///
    /// When configured, job completions are collected in a batch and flushed
    /// together after the specified delay. This reduces the number of SQL
    /// round trips and can significantly improve throughput.
    ///
    /// # Arguments
    ///
    /// * `delay` - The duration to wait before flushing the batch.
    ///   A small value like 1-5ms is recommended.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use graphile_worker::WorkerOptions;
    /// # use std::time::Duration;
    /// let worker = WorkerOptions::default()
    ///     .complete_job_batch_delay(Duration::from_millis(5));
    /// ```
    pub fn complete_job_batch_delay(mut self, delay: Duration) -> Self {
        self.complete_job_batch_delay = Some(delay);
        self
    }

    /// Sets the delay before flushing batched job failures.
    ///
    /// When configured, permanent job failures are collected and flushed
    /// together after the specified delay. Retryable failures are still
    /// processed individually to ensure proper backoff timing.
    ///
    /// # Arguments
    ///
    /// * `delay` - The duration to wait before flushing the batch.
    ///   A small value like 1-5ms is recommended.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use graphile_worker::WorkerOptions;
    /// # use std::time::Duration;
    /// let worker = WorkerOptions::default()
    ///     .fail_job_batch_delay(Duration::from_millis(5));
    /// ```
    pub fn fail_job_batch_delay(mut self, delay: Duration) -> Self {
        self.fail_job_batch_delay = Some(delay);
        self
    }
}
