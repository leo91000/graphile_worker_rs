use crate::local_queue::LocalQueueConfig;

use super::WorkerOptions;

impl WorkerOptions {
    /// Enables the LocalQueue with the specified configuration.
    ///
    /// LocalQueue batch-fetches jobs from the database to reduce DB load,
    /// trading latency for throughput. Jobs are cached locally and distributed
    /// to workers without additional database queries until the cache is empty.
    ///
    /// # Arguments
    /// * `config` - The LocalQueue configuration (size and TTL)
    ///
    /// # Note
    /// When LocalQueue is enabled, jobs may experience slightly higher latency
    /// as they wait in the local cache. The cache has a TTL after which
    /// unclaimed jobs are returned to the database.
    ///
    /// `LocalQueueConfig::queue_count` can be raised above 1 to run multiple
    /// independent local queues in the same worker. `size` is applied per queue,
    /// so total local capacity is `size * queue_count`.
    /// There is no universal best setting for throughput; benchmark realistic
    /// jobs with your own PostgreSQL latency, pool size, worker concurrency, and
    /// local queue settings before tuning this in production.
    ///
    /// Workers with `forbidden_flags` will bypass the LocalQueue and fetch
    /// jobs directly from the database.
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerOptions, LocalQueueConfig, RefetchDelayConfig};
    /// # use std::time::Duration;
    ///
    /// let options = WorkerOptions::default()
    ///     .local_queue(
    ///         LocalQueueConfig::default()
    ///             .with_size(100)
    ///             .with_queue_count(2)
    ///             .with_ttl(Duration::from_secs(300))
    ///             .with_refetch_delay(
    ///                 RefetchDelayConfig::default()
    ///                     .with_duration(Duration::from_millis(100))
    ///                     .with_threshold(10)
    ///                     .with_max_abort_threshold(500),
    ///             ),
    ///     );
    /// ```
    pub fn local_queue(mut self, config: LocalQueueConfig) -> Self {
        self.local_queue_config = Some(config);
        self
    }
}
