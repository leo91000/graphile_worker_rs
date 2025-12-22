use chrono::{prelude::*, Duration};
use once_cell::sync::Lazy;

pub(crate) static ONE_MINUTE: Lazy<Duration> = Lazy::new(|| Duration::minutes(1));

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
