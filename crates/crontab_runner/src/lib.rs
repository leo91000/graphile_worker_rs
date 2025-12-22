use std::cmp::Ordering;

use backfill::register_and_backfill_items;
use chrono::prelude::*;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_lifecycle_hooks::{CronJobScheduledContext, CronTickContext, ObserverFn};
use graphile_worker_shutdown_signal::ShutdownSignal;
use sqlx::PgExecutor;
use tracing::{debug, warn};

pub use crate::clock::mock::MockClock;
pub use crate::clock::Clock;
pub use crate::sql::KnownCrontab;
pub use crate::sql::ScheduleCronJobError;
use crate::{
    clock::SystemClock,
    sql::{schedule_cron_jobs, CrontabJob},
    utils::{round_date_minute, ONE_MINUTE},
};

mod backfill;
pub mod clock;
mod sql;
mod utils;

pub async fn cron_main<'e>(
    executor: impl PgExecutor<'e> + Clone,
    escaped_schema: &str,
    crontabs: &[Crontab],
    use_local_time: bool,
    shutdown_signal: ShutdownSignal,
    on_cron_tick: &[ObserverFn<CronTickContext>],
    on_cron_job_scheduled: &[ObserverFn<CronJobScheduledContext>],
) -> Result<(), ScheduleCronJobError> {
    CronRunner::new(executor, escaped_schema, crontabs)
        .use_local_time(use_local_time)
        .on_cron_tick(on_cron_tick)
        .on_cron_job_scheduled(on_cron_job_scheduled)
        .run(shutdown_signal)
        .await
}

pub struct CronRunner<'a, E, C = SystemClock> {
    executor: E,
    escaped_schema: &'a str,
    crontabs: &'a [Crontab],
    use_local_time: bool,
    on_cron_tick: &'a [ObserverFn<CronTickContext>],
    on_cron_job_scheduled: &'a [ObserverFn<CronJobScheduledContext>],
    clock: C,
}

impl<'a, E> CronRunner<'a, E, SystemClock> {
    pub fn new(executor: E, escaped_schema: &'a str, crontabs: &'a [Crontab]) -> Self {
        Self {
            executor,
            escaped_schema,
            crontabs,
            use_local_time: false,
            on_cron_tick: &[],
            on_cron_job_scheduled: &[],
            clock: SystemClock,
        }
    }
}

impl<'a, E, C: Clock> CronRunner<'a, E, C> {
    pub fn use_local_time(mut self, use_local_time: bool) -> Self {
        self.use_local_time = use_local_time;
        self
    }

    pub fn on_cron_tick(mut self, hooks: &'a [ObserverFn<CronTickContext>]) -> Self {
        self.on_cron_tick = hooks;
        self
    }

    pub fn on_cron_job_scheduled(
        mut self,
        hooks: &'a [ObserverFn<CronJobScheduledContext>],
    ) -> Self {
        self.on_cron_job_scheduled = hooks;
        self
    }

    pub fn with_clock<C2: Clock>(self, clock: C2) -> CronRunner<'a, E, C2> {
        CronRunner {
            executor: self.executor,
            escaped_schema: self.escaped_schema,
            crontabs: self.crontabs,
            use_local_time: self.use_local_time,
            on_cron_tick: self.on_cron_tick,
            on_cron_job_scheduled: self.on_cron_job_scheduled,
            clock,
        }
    }

    pub async fn run<'e>(
        self,
        mut shutdown_signal: ShutdownSignal,
    ) -> Result<(), ScheduleCronJobError>
    where
        E: PgExecutor<'e> + Clone,
    {
        let start = self.clock.now();
        debug!(start = ?start, "cron:starting");

        register_and_backfill_items(
            self.executor.clone(),
            self.escaped_schema,
            self.crontabs,
            &start,
            self.use_local_time,
        )
        .await?;
        debug!(start = ?start, "cron:started");

        let mut ts = round_date_minute(start, true);

        loop {
            tokio::select! {
                _ = self.clock.sleep_until(ts) => (),
                _ = (&mut shutdown_signal) => break Ok(()),
            };

            let current_ts = round_date_minute(self.clock.now(), false);
            let ts_delta = current_ts - ts;

            match ts_delta.num_minutes().cmp(&0) {
                Ordering::Less => {
                    warn!(
                        "Cron fired {}s too early (clock skew?); rescheduling",
                        -ts_delta.num_seconds()
                    );
                    continue;
                }
                Ordering::Greater => {
                    warn!(
                        "Cron fired too late; catching up {}m{}s behind",
                        ts_delta.num_minutes(),
                        ts_delta.num_seconds() % 60
                    );
                }
                _ => {}
            }

            let tick_ctx = CronTickContext {
                timestamp: ts.with_timezone(&Utc),
                crontabs: self.crontabs.to_vec(),
            };
            for hook in self.on_cron_tick {
                hook(tick_ctx.clone()).await;
            }

            let mut jobs: Vec<CrontabJob> = vec![];

            for cron in self.crontabs {
                if cron.should_run_at(&ts.naive_local()) {
                    jobs.push(CrontabJob::for_cron(cron, &ts, false));

                    let scheduled_ctx = CronJobScheduledContext {
                        crontab: cron.clone(),
                        scheduled_at: ts.with_timezone(&Utc),
                    };
                    for hook in self.on_cron_job_scheduled {
                        hook(scheduled_ctx.clone()).await;
                    }
                }
            }

            if !jobs.is_empty() {
                debug!(nb_jobs = jobs.len(), at = ?ts, "cron:schedule");
                schedule_cron_jobs(
                    self.executor.clone(),
                    &jobs,
                    &ts,
                    self.escaped_schema,
                    self.use_local_time,
                )
                .await?;
                debug!(nb_jobs = jobs.len(), at = ?ts, "cron:scheduled");
            }

            ts += *ONE_MINUTE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::mock::MockClock;
    use chrono::Duration;

    fn test_time() -> DateTime<Local> {
        Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap()
    }

    #[test]
    fn test_round_date_minute_no_round_up() {
        let time = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 45).unwrap();
        let rounded = round_date_minute(time, false);
        assert_eq!(rounded.minute(), 30);
        assert_eq!(rounded.second(), 0);
        assert_eq!(rounded.nanosecond(), 0);
    }

    #[test]
    fn test_round_date_minute_with_round_up() {
        let time = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 45).unwrap();
        let rounded = round_date_minute(time, true);
        assert_eq!(rounded.minute(), 31);
        assert_eq!(rounded.second(), 0);
        assert_eq!(rounded.nanosecond(), 0);
    }

    #[test]
    fn test_clock_skew_too_early_detection() {
        let scheduled = Local.with_ymd_and_hms(2024, 1, 15, 10, 31, 0).unwrap();
        let current = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let current_rounded = round_date_minute(current, false);
        let ts_delta = current_rounded - scheduled;

        assert!(
            ts_delta.num_minutes() < 0,
            "When current time is before scheduled, delta should be negative"
        );
        assert!(
            matches!(ts_delta.num_minutes().cmp(&0), Ordering::Less),
            "Should match Ordering::Less for too early case"
        );
    }

    #[test]
    fn test_clock_skew_too_late_detection() {
        let scheduled = Local.with_ymd_and_hms(2024, 1, 15, 10, 31, 0).unwrap();
        let current = Local.with_ymd_and_hms(2024, 1, 15, 10, 45, 0).unwrap();

        let current_rounded = round_date_minute(current, false);
        let ts_delta = current_rounded - scheduled;

        assert!(
            ts_delta.num_minutes() > 0,
            "When current time is after scheduled, delta should be positive"
        );
        assert!(
            matches!(ts_delta.num_minutes().cmp(&0), Ordering::Greater),
            "Should match Ordering::Greater for too late case"
        );
    }

    #[test]
    fn test_clock_skew_on_time_detection() {
        let scheduled = Local.with_ymd_and_hms(2024, 1, 15, 10, 31, 0).unwrap();
        let current = Local.with_ymd_and_hms(2024, 1, 15, 10, 31, 30).unwrap();

        let current_rounded = round_date_minute(current, false);
        let ts_delta = current_rounded - scheduled;

        assert_eq!(
            ts_delta.num_minutes(),
            0,
            "When current time equals scheduled (same minute), delta should be zero"
        );
        assert!(
            matches!(ts_delta.num_minutes().cmp(&0), Ordering::Equal),
            "Should match Ordering::Equal for on-time case"
        );
    }

    #[test]
    fn test_system_sleep_scenario() {
        let scheduled = Local.with_ymd_and_hms(2024, 1, 15, 10, 1, 0).unwrap();
        let after_sleep = Local.with_ymd_and_hms(2024, 1, 15, 11, 25, 0).unwrap();

        let current_rounded = round_date_minute(after_sleep, false);
        let ts_delta = current_rounded - scheduled;

        assert_eq!(ts_delta.num_minutes(), 84, "Should be 84 minutes behind");
        assert!(
            matches!(ts_delta.num_minutes().cmp(&0), Ordering::Greater),
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
}
