use std::time::Duration;

use thiserror::Error;

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
