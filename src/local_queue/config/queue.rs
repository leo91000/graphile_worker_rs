use std::time::Duration;

use derive_builder::Builder;

use super::error::LocalQueueConfigError;
use super::refetch::RefetchDelayConfig;

const DEFAULT_LOCAL_QUEUE_SIZE: usize = 100;
const DEFAULT_LOCAL_QUEUE_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Builder)]
#[builder(build_fn(private, name = "build_internal"), pattern = "owned")]
pub struct LocalQueueConfig {
    /// Maximum number of jobs each local queue may fetch and hold at once.
    ///
    /// When `queue_count` is greater than 1, this value is per local queue.
    /// For example, `size = 1_250` and `queue_count = 4` allows up to 5,000
    /// jobs to be locked locally across the worker.
    #[builder(default = "DEFAULT_LOCAL_QUEUE_SIZE")]
    pub size: usize,
    /// How long locally fetched jobs may stay unclaimed before being returned to the database.
    #[builder(default = "DEFAULT_LOCAL_QUEUE_TTL")]
    pub ttl: Duration,
    /// Optional delay strategy used when a fetch returns fewer jobs than requested.
    #[builder(default, setter(strip_option))]
    pub refetch_delay: Option<RefetchDelayConfig>,
    /// Number of independent local queues to run inside this worker.
    ///
    /// Multiple queues can improve throughput for very small high-volume jobs by
    /// letting the worker fetch several batches in parallel. This also increases
    /// the maximum number of jobs locked locally to `size * queue_count`, so keep
    /// `size` lower when increasing this value.
    ///
    /// If this is greater than the worker concurrency, only `concurrency` queues
    /// are started because every local queue needs at least one worker draining it.
    ///
    /// Throughput depends on the job payload, handler cost, PostgreSQL latency,
    /// connection pool size, worker concurrency, and local queue settings. There
    /// is no universal best value; benchmark realistic workloads before changing
    /// this in production.
    ///
    /// Defaults to 1, which preserves the original single-local-queue behavior.
    #[builder(default = "1")]
    pub queue_count: usize,
}

impl Default for LocalQueueConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl LocalQueueConfig {
    pub fn builder() -> LocalQueueConfigBuilder {
        LocalQueueConfigBuilder::default()
    }

    pub fn validate(&self, poll_interval: Duration) -> Result<(), LocalQueueConfigError> {
        if let Some(refetch_delay) = self.refetch_delay.as_ref() {
            if refetch_delay.duration > poll_interval {
                return Err(LocalQueueConfigError::RefetchDelayExceedsPollInterval {
                    duration: refetch_delay.duration,
                    poll_interval,
                });
            }
        }

        if self.size == 0 {
            return Err(LocalQueueConfigError::EmptySize);
        }

        if self.queue_count == 0 {
            return Err(LocalQueueConfigError::EmptyQueueCount);
        }

        if self.size > i32::MAX as usize {
            return Err(LocalQueueConfigError::SizeTooLarge {
                size: self.size,
                max: i32::MAX,
            });
        }

        Ok(())
    }

    pub fn with_size(self, size: usize) -> Self {
        Self { size, ..self }
    }

    pub fn with_ttl(self, ttl: Duration) -> Self {
        Self { ttl, ..self }
    }

    pub fn with_refetch_delay(self, refetch_delay: RefetchDelayConfig) -> Self {
        Self {
            refetch_delay: Some(refetch_delay),
            ..self
        }
    }

    /// Sets how many independent local queues this worker should run.
    ///
    /// `size` is applied to each queue, so total local capacity is
    /// `size * queue_count`. Higher values increase parallel fetch capacity but
    /// can also lock more jobs locally and increase database load.
    ///
    /// If this is greater than worker concurrency, it is capped at concurrency.
    /// There is no single best value for all workloads, so benchmark realistic
    /// jobs before relying on a throughput tuning.
    pub fn with_queue_count(self, queue_count: usize) -> Self {
        Self {
            queue_count,
            ..self
        }
    }
}

impl LocalQueueConfigBuilder {
    pub fn build(self) -> LocalQueueConfig {
        self.build_internal()
            .expect("All fields have defaults, build should never fail")
    }
}
