use time::{Duration, OffsetDateTime, PrimitiveDateTime, UtcOffset};

pub(crate) const ONE_MINUTE: Duration = Duration::minutes(1);

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

pub(crate) fn round_date_minute(mut datetime: OffsetDateTime, round_up: bool) -> OffsetDateTime {
    datetime = datetime
        .replace_second(0)
        .unwrap()
        .replace_nanosecond(0)
        .unwrap();
    if round_up {
        datetime += ONE_MINUTE;
    }
    datetime
}

pub(crate) fn calendar_time(local_time: OffsetDateTime, use_local_time: bool) -> PrimitiveDateTime {
    let datetime = if use_local_time {
        local_time
    } else {
        local_time.to_offset(UtcOffset::UTC)
    };
    PrimitiveDateTime::new(datetime.date(), datetime.time())
}
