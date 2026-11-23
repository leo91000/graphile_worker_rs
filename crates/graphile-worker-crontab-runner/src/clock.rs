use graphile_worker_runtime as runtime;
use std::future::Future;
use time::{Duration, OffsetDateTime, UtcOffset};

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;

    fn sleep_until(&self, datetime: OffsetDateTime) -> impl Future<Output = ()> + Send;

    /// Express a scheduled instant in this clock's local time.
    ///
    /// By default, the instant retains its existing offset. Clocks that observe
    /// timezone transitions can resolve the offset at the supplied instant.
    fn local_time(&self, datetime: OffsetDateTime) -> OffsetDateTime {
        datetime
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

    fn local_time(&self, datetime: OffsetDateTime) -> OffsetDateTime {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        refresh_timezone();

        let offset = UtcOffset::local_offset_at(datetime).unwrap_or(UtcOffset::UTC);
        datetime.to_offset(offset)
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn refresh_timezone() {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    unsafe extern "C" {
        fn tzset();
    }

    static LAST_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST_REFRESH
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if last.is_some_and(|last| last.elapsed() < Duration::from_secs(1)) {
        return;
    }

    // glibc's localtime_r can retain its cache after /etc/localtime changes.
    // SAFETY: tzset takes no arguments, is thread-safe on glibc, and we do not read its globals.
    unsafe { tzset() };
    *last = Some(Instant::now());
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
