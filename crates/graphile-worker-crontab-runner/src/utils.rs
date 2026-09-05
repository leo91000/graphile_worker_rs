use chrono::{prelude::*, Duration};
use once_cell::sync::Lazy;

pub(crate) static ONE_MINUTE: Lazy<Duration> = Lazy::new(|| Duration::minutes(1));

/// Give the enclosing shutdown future a chance to run even when all work is ready.
pub(crate) async fn yield_now() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            return std::task::Poll::Ready(());
        }
        yielded = true;
        cx.waker().wake_by_ref();
        std::task::Poll::Pending
    })
    .await;
}

pub(crate) fn round_date_minute<Tz: TimeZone>(
    mut datetime: DateTime<Tz>,
    round_up: bool,
) -> DateTime<Tz> {
    datetime = datetime.with_second(0).unwrap().with_nanosecond(0).unwrap();
    if round_up {
        datetime += Duration::minutes(1);
    }
    datetime
}
