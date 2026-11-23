use std::cmp::Ordering;

use time::{macros::datetime, Duration, OffsetDateTime};

use crate::clock::mock::MockClock;
use crate::clock::Clock;
use crate::utils::round_date_minute;

#[path = "tests/shutdown.rs"]
mod shutdown;

fn test_time() -> OffsetDateTime {
    datetime!(2024-01-15 10:30:00 UTC)
}

#[test]
fn test_round_date_minute_no_round_up() {
    let time = datetime!(2024-01-15 10:30:45 UTC);
    let rounded = round_date_minute(time, false);
    assert_eq!(rounded.minute(), 30);
    assert_eq!(rounded.second(), 0);
    assert_eq!(rounded.nanosecond(), 0);
}

#[test]
fn test_round_date_minute_with_round_up() {
    let time = datetime!(2024-01-15 10:30:45 UTC);
    let rounded = round_date_minute(time, true);
    assert_eq!(rounded.minute(), 31);
    assert_eq!(rounded.second(), 0);
    assert_eq!(rounded.nanosecond(), 0);
}

#[test]
fn test_clock_skew_too_early_detection() {
    let scheduled = datetime!(2024-01-15 10:31:00 UTC);
    let current = datetime!(2024-01-15 10:30:00 UTC);

    let current_rounded = round_date_minute(current, false);
    let ts_delta = current_rounded - scheduled;

    assert!(
        ts_delta.whole_minutes() < 0,
        "When current time is before scheduled, delta should be negative"
    );
    assert!(
        matches!(ts_delta.whole_minutes().cmp(&0), Ordering::Less),
        "Should match Ordering::Less for too early case"
    );
}

#[test]
fn test_clock_skew_too_late_detection() {
    let scheduled = datetime!(2024-01-15 10:31:00 UTC);
    let current = datetime!(2024-01-15 10:45:00 UTC);

    let current_rounded = round_date_minute(current, false);
    let ts_delta = current_rounded - scheduled;

    assert!(
        ts_delta.whole_minutes() > 0,
        "When current time is after scheduled, delta should be positive"
    );
    assert!(
        matches!(ts_delta.whole_minutes().cmp(&0), Ordering::Greater),
        "Should match Ordering::Greater for too late case"
    );
}

#[test]
fn test_clock_skew_on_time_detection() {
    let scheduled = datetime!(2024-01-15 10:31:00 UTC);
    let current = datetime!(2024-01-15 10:31:30 UTC);

    let current_rounded = round_date_minute(current, false);
    let ts_delta = current_rounded - scheduled;

    assert_eq!(
        ts_delta.whole_minutes(),
        0,
        "When current time equals scheduled (same minute), delta should be zero"
    );
    assert!(
        matches!(ts_delta.whole_minutes().cmp(&0), Ordering::Equal),
        "Should match Ordering::Equal for on-time case"
    );
}

#[test]
fn test_system_sleep_scenario() {
    let scheduled = datetime!(2024-01-15 10:01:00 UTC);
    let after_sleep = datetime!(2024-01-15 11:25:00 UTC);

    let current_rounded = round_date_minute(after_sleep, false);
    let ts_delta = current_rounded - scheduled;

    assert_eq!(ts_delta.whole_minutes(), 84, "Should be 84 minutes behind");
    assert!(
        matches!(ts_delta.whole_minutes().cmp(&0), Ordering::Greater),
        "Should match Ordering::Greater for catch-up mode"
    );
}

#[tokio::test]
async fn test_mock_clock_now() {
    let initial = test_time();
    let clock = MockClock::new(initial);

    assert_eq!(clock.now(), initial);
}

#[tokio::test]
async fn test_mock_clock_set_time() {
    let initial = test_time();
    let clock = MockClock::new(initial);

    let new_time = initial + Duration::hours(2);
    clock.set_time(new_time);

    assert_eq!(clock.now(), new_time);
}

#[tokio::test]
async fn test_mock_clock_advance() {
    let initial = test_time();
    let clock = MockClock::new(initial);

    clock.advance(Duration::minutes(30));

    assert_eq!(clock.now(), initial + Duration::minutes(30));
}

#[tokio::test]
async fn test_mock_clock_sleep_until_already_passed() {
    let initial = test_time();
    let clock = MockClock::new(initial);

    let past_time = initial - Duration::hours(1);
    clock.sleep_until(past_time).await;
}

#[tokio::test]
async fn test_mock_clock_sleep_until_wakes_on_time_advance() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let initial = test_time();
    let clock = Arc::new(MockClock::new(initial));
    let target = initial + Duration::minutes(5);

    let clock_clone = Arc::clone(&clock);
    let woke_up = Arc::new(Mutex::new(false));
    let woke_up_clone = Arc::clone(&woke_up);

    let handle = tokio::spawn(async move {
        clock_clone.sleep_until(target).await;
        *woke_up_clone.lock().await = true;
    });

    tokio::task::yield_now().await;
    assert!(!*woke_up.lock().await, "Should not have woken up yet");

    clock.set_time(target);
    handle.await.unwrap();

    assert!(
        *woke_up.lock().await,
        "Should have woken up after time advance"
    );
}
