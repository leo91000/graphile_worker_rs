use std::time::Duration;

use super::config::{calculate_retry_delay, RetryOptions};
use super::*;

#[test]
fn retry_delay_applies_cap_and_jitter() {
    let options = RetryOptions {
        max_attempts: 3,
        min_delay_ms: 1_000,
        max_delay_ms: 10,
        multiplier: 2.0,
    };

    let delay = calculate_retry_delay(4, &options);
    assert!(delay >= Duration::from_millis(5));
    assert!(delay <= Duration::from_millis(10));
}

#[test]
fn local_queue_config_builders_and_mutators() {
    let refetch_delay = RefetchDelayConfig::default()
        .with_duration(Duration::from_millis(50))
        .with_threshold(2)
        .with_max_abort_threshold(9);

    assert_eq!(refetch_delay.duration, Duration::from_millis(50));
    assert_eq!(refetch_delay.threshold, 2);
    assert_eq!(refetch_delay.max_abort_threshold, Some(9));

    let built_refetch_delay = RefetchDelayConfig::builder()
        .duration(Duration::from_millis(25))
        .threshold(1)
        .max_abort_threshold(4)
        .build();
    assert_eq!(built_refetch_delay.duration, Duration::from_millis(25));
    assert_eq!(built_refetch_delay.threshold, 1);
    assert_eq!(built_refetch_delay.max_abort_threshold, Some(4));

    let config = LocalQueueConfig::default()
        .with_size(7)
        .with_ttl(Duration::from_secs(3))
        .with_refetch_delay(refetch_delay.clone())
        .with_queue_count(3);
    assert_eq!(config.size, 7);
    assert_eq!(config.ttl, Duration::from_secs(3));
    assert!(config.refetch_delay.is_some());
    assert_eq!(config.queue_count, 3);

    let built_config = LocalQueueConfig::builder()
        .size(8)
        .ttl(Duration::from_secs(4))
        .refetch_delay(built_refetch_delay)
        .queue_count(2)
        .build();
    assert_eq!(built_config.size, 8);
    assert_eq!(built_config.ttl, Duration::from_secs(4));
    assert!(built_config.refetch_delay.is_some());
    assert_eq!(built_config.queue_count, 2);
}

#[test]
fn local_queue_config_validation_rejects_invalid_values() {
    assert_eq!(
        LocalQueueConfig::default()
            .with_size(0)
            .validate(Duration::from_secs(1)),
        Err(LocalQueueConfigError::EmptySize)
    );
    assert_eq!(
        LocalQueueConfig::default()
            .with_queue_count(0)
            .validate(Duration::from_secs(1)),
        Err(LocalQueueConfigError::EmptyQueueCount)
    );
    assert!(matches!(
        LocalQueueConfig::default()
            .with_refetch_delay(RefetchDelayConfig::default().with_duration(Duration::from_secs(2)))
            .validate(Duration::from_secs(1)),
        Err(LocalQueueConfigError::RefetchDelayExceedsPollInterval { .. })
    ));
}
