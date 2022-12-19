use chrono::{prelude::*, Duration};
use once_cell::sync::Lazy;

pub(crate) static DURATION_ZERO: Lazy<Duration> = Lazy::new(Duration::zero);
pub(crate) static ONE_MINUTE: Lazy<Duration> = Lazy::new(|| Duration::minutes(1));

pub(crate) fn round_date_minute<Tz: TimeZone>(
    mut datetime: DateTime<Tz>,
    round_up: bool,
) -> DateTime<Tz> {
    datetime = datetime.with_second(0).unwrap();
    if round_up {
        datetime += Duration::minutes(1);
    }
    datetime
}

pub(crate) async fn sleep_until<Tz: TimeZone>(datetime: DateTime<Tz>) {
    let dur = datetime.with_timezone(&Local) - Local::now();
    if dur < *DURATION_ZERO {
        return;
    }

    tokio::time::sleep(dur.to_std().unwrap()).await;
}
