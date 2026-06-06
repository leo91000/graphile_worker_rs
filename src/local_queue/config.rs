use std::time::Duration;

use derive_builder::Builder;
use rand::RngExt;
use thiserror::Error;

pub(super) const DEFAULT_LOCAL_QUEUE_SIZE: usize = 100;
pub(super) const DEFAULT_LOCAL_QUEUE_TTL: Duration = Duration::from_secs(5 * 60);

pub(super) struct RetryOptions {
    pub(super) max_attempts: u32,
    pub(super) min_delay_ms: u64,
    pub(super) max_delay_ms: u64,
    pub(super) multiplier: f64,
}

pub(super) const RETURN_JOBS_RETRY_OPTIONS: RetryOptions = RetryOptions {
    max_attempts: 20,
    min_delay_ms: 200,
    max_delay_ms: 30_000,
    multiplier: 1.5,
};

pub(super) fn calculate_retry_delay(attempt: u32, options: &RetryOptions) -> Duration {
    let base = options.min_delay_ms as f64 * options.multiplier.powi(attempt as i32);
    let capped = base.min(options.max_delay_ms as f64);
    let jittered = capped * (0.5 + rand::rng().random::<f64>() * 0.5);
    Duration::from_millis(jittered as u64)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalQueueConfigError {
    #[error("local_queue.refetch_delay.duration ({duration:?}) must not be larger than poll_interval ({poll_interval:?})")]
    RefetchDelayExceedsPollInterval {
        duration: Duration,
        poll_interval: Duration,
    },
    #[error("local_queue.size must be greater than 0")]
    EmptySize,
    #[error("local_queue.queue_count must be greater than 0")]
    EmptyQueueCount,
    #[error("local_queue.size ({size}) must not exceed i32::MAX ({max})")]
    SizeTooLarge { size: usize, max: i32 },
}

#[derive(Debug, Clone, Builder)]
#[builder(build_fn(private, name = "build_internal"), pattern = "owned")]
pub struct RefetchDelayConfig {
    #[builder(default = "Duration::from_millis(100)")]
    pub duration: Duration,
    #[builder(default = "0")]
    pub threshold: usize,
    #[builder(default, setter(strip_option))]
    pub max_abort_threshold: Option<usize>,
}

impl Default for RefetchDelayConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl RefetchDelayConfig {
    pub fn builder() -> RefetchDelayConfigBuilder {
        RefetchDelayConfigBuilder::default()
    }

    pub fn with_duration(self, duration: Duration) -> Self {
        Self { duration, ..self }
    }

    pub fn with_threshold(self, threshold: usize) -> Self {
        Self { threshold, ..self }
    }

    pub fn with_max_abort_threshold(self, max_abort_threshold: usize) -> Self {
        Self {
            max_abort_threshold: Some(max_abort_threshold),
            ..self
        }
    }
}

impl RefetchDelayConfigBuilder {
    pub fn build(self) -> RefetchDelayConfig {
        self.build_internal()
            .expect("All fields have defaults, build should never fail")
    }
}

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
