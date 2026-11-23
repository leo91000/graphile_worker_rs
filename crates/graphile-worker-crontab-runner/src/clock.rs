use graphile_worker_runtime as runtime;
use std::future::Future;
use time::{Duration, OffsetDateTime, UtcOffset};

#[cfg(windows)]
mod windows;

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;

    fn sleep_until(&self, datetime: OffsetDateTime) -> impl Future<Output = ()> + Send;

    /// Express a scheduled instant in this clock's local time.
    ///
    /// The default uses the host timezone, including transitions. Clocks with
    /// their own timezone or a fixed offset can override this method.
    fn local_time(&self, datetime: OffsetDateTime) -> OffsetDateTime {
        let offset = local_offset_at(datetime);
        datetime.checked_to_offset(offset).unwrap_or(datetime)
    }
}

#[derive(Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        self.local_time(OffsetDateTime::now_utc())
    }

    async fn sleep_until(&self, datetime: OffsetDateTime) {
        let duration = datetime - OffsetDateTime::now_utc();
        let Ok(std_dur) = std::time::Duration::try_from(duration) else {
            return;
        };
        runtime::sleep(std_dur).await;
    }
}

fn local_offset_at(datetime: OffsetDateTime) -> UtcOffset {
    #[cfg(unix)]
    if let Some(offset) = host_offset_at(datetime) {
        return offset;
    }

    #[cfg(windows)]
    let offset = windows::offset_at(datetime);
    #[cfg(not(windows))]
    let offset = UtcOffset::local_offset_at(datetime).ok();
    offset.unwrap_or(UtcOffset::UTC)
}

#[cfg(unix)]
fn host_offset_at(datetime: OffsetDateTime) -> Option<UtcOffset> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    struct CachedTimezone {
        setting: Option<String>,
        loaded: Instant,
        timezone: Option<tz::TimeZone>,
    }

    const SETTINGS: tz::TimeZoneSettings<'static> = tz::TimeZoneSettings::new(
        &[
            "/usr/share/zoneinfo",
            "/share/zoneinfo",
            "/etc/zoneinfo",
            "/usr/share/lib/zoneinfo",
        ],
        tz::TimeZoneSettings::DEFAULT_READ_FILE_FN,
    );

    // Reload Unix timezone rules because libc can retain stale timezone state.
    static HOST_ZONE: Mutex<Option<CachedTimezone>> = Mutex::new(None);
    let setting = std::env::var("TZ").ok();
    let mut cached = HOST_ZONE.lock().unwrap_or_else(|error| error.into_inner());
    if cached.as_ref().is_none_or(|zone| {
        zone.setting != setting || zone.loaded.elapsed() >= Duration::from_secs(1)
    }) {
        let timezone = match setting.as_deref() {
            Some("") => Some(tz::TimeZone::utc()),
            Some(setting) => SETTINGS.parse_posix_tz(setting).ok(),
            None => SETTINGS.parse_local().ok(),
        }
        .or_else(|| {
            SETTINGS
                .parse_posix_tz(&iana_time_zone::get_timezone().ok()?)
                .ok()
        });
        *cached = Some(CachedTimezone {
            setting,
            loaded: Instant::now(),
            timezone,
        });
    }

    let offset = cached
        .as_ref()?
        .timezone
        .as_ref()?
        .find_local_time_type(datetime.unix_timestamp())
        .ok()?
        .ut_offset();
    UtcOffset::from_whole_seconds(offset).ok()
}

pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex};

    pub struct MockClock {
        current_time: Arc<Mutex<OffsetDateTime>>,
        wake_notify: Arc<runtime::Notify>,
    }

    impl MockClock {
        pub fn new(initial_time: OffsetDateTime) -> Self {
            Self {
                current_time: Arc::new(Mutex::new(initial_time)),
                wake_notify: Arc::new(runtime::Notify::new()),
            }
        }

        pub fn set_time(&self, time: OffsetDateTime) {
            *self.current_time.lock().unwrap() = time;
            self.wake_notify.notify_waiters();
        }

        pub fn advance(&self, duration: Duration) {
            let mut time = self.current_time.lock().unwrap();
            *time += duration;
            drop(time);
            self.wake_notify.notify_waiters();
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> OffsetDateTime {
            *self.current_time.lock().unwrap()
        }

        async fn sleep_until(&self, datetime: OffsetDateTime) {
            loop {
                let now = self.now();
                if now >= datetime {
                    return;
                }
                self.wake_notify.notified().await;
            }
        }
    }

    impl Clock for Arc<MockClock> {
        fn now(&self) -> OffsetDateTime {
            MockClock::now(self)
        }

        async fn sleep_until(&self, datetime: OffsetDateTime) {
            MockClock::sleep_until(self, datetime).await
        }
    }
}
