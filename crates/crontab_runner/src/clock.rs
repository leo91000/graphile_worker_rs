use chrono::{DateTime, Local};
use std::future::Future;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Local>;

    fn sleep_until(&self, datetime: DateTime<Local>) -> impl Future<Output = ()> + Send;
}

#[derive(Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }

    async fn sleep_until(&self, datetime: DateTime<Local>) {
        let dur = datetime - Local::now();
        let Ok(std_dur) = dur.to_std() else { return };
        tokio::time::sleep(std_dur).await;
    }
}

pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::sync::Notify;

    pub struct MockClock {
        current_time: Arc<Mutex<DateTime<Local>>>,
        wake_notify: Arc<Notify>,
    }

    impl MockClock {
        pub fn new(initial_time: DateTime<Local>) -> Self {
            Self {
                current_time: Arc::new(Mutex::new(initial_time)),
                wake_notify: Arc::new(Notify::new()),
            }
        }

        pub fn set_time(&self, time: DateTime<Local>) {
            *self.current_time.lock().unwrap() = time;
            self.wake_notify.notify_waiters();
        }

        pub fn advance(&self, duration: chrono::Duration) {
            let mut time = self.current_time.lock().unwrap();
            *time += duration;
            drop(time);
            self.wake_notify.notify_waiters();
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> DateTime<Local> {
            *self.current_time.lock().unwrap()
        }

        async fn sleep_until(&self, datetime: DateTime<Local>) {
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
        fn now(&self) -> DateTime<Local> {
            MockClock::now(self)
        }

        async fn sleep_until(&self, datetime: DateTime<Local>) {
            MockClock::sleep_until(self, datetime).await
        }
    }
}
