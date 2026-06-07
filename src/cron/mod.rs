mod builder;

use chrono::Weekday;
use graphile_worker_crontab_types::{CrontabTimer, CrontabTimerError};
use graphile_worker_task_handler::TaskHandler;

pub use builder::CronBuilder;

/// Namespace for typed cron schedule builders.
///
/// `Cron` creates [`graphile_worker_crontab_types::Crontab`] values from Rust types instead of
/// crontab strings. The task identifier is taken from `T::IDENTIFIER`, so it
/// stays aligned with the registered [`TaskHandler`].
///
/// ```rust
/// # use graphile_worker::{Cron, CrontabFill, WorkerOptions};
/// # use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
/// # use serde::{Deserialize, Serialize};
/// #
/// # #[derive(Deserialize, Serialize)]
/// # struct SendDailyReport;
/// #
/// # impl TaskHandler for SendDailyReport {
/// #     const IDENTIFIER: &'static str = "send_daily_report";
/// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
/// # }
/// #
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let options = WorkerOptions::default()
///     .define_job::<SendDailyReport>()
///     .with_cron(
///         Cron::daily_at::<SendDailyReport>(8, 0)?
///             .fill(CrontabFill::hours(1)),
///     );
/// # let _ = options;
/// # Ok(())
/// # }
/// ```
pub struct Cron;

impl Cron {
    /// Build a typed cron entry from a custom timer.
    pub fn from_timer<T: TaskHandler>(timer: CrontabTimer) -> CronBuilder<T> {
        CronBuilder::new(timer)
    }

    /// Run the task every minute.
    pub fn every_minute<T: TaskHandler>() -> CronBuilder<T> {
        Self::from_timer(CrontabTimer::every_minute())
    }

    /// Run the task every `step` minutes.
    pub fn every_n_minutes<T: TaskHandler>(step: u32) -> Result<CronBuilder<T>, CrontabTimerError> {
        Ok(Self::from_timer(CrontabTimer::every_n_minutes(step)?))
    }

    /// Run the task once per hour at the given minute.
    pub fn hourly_at<T: TaskHandler>(minute: u32) -> Result<CronBuilder<T>, CrontabTimerError> {
        Ok(Self::from_timer(CrontabTimer::hourly_at(minute)?))
    }

    /// Run the task once per day at the given UTC hour and minute.
    pub fn daily_at<T: TaskHandler>(
        hour: u32,
        minute: u32,
    ) -> Result<CronBuilder<T>, CrontabTimerError> {
        Ok(Self::from_timer(CrontabTimer::daily_at(hour, minute)?))
    }

    /// Run the task once per week on the given weekday, UTC hour and minute.
    pub fn weekly_on<T: TaskHandler>(
        weekday: Weekday,
        hour: u32,
        minute: u32,
    ) -> Result<CronBuilder<T>, CrontabTimerError> {
        Ok(Self::from_timer(CrontabTimer::weekly_on(
            weekday, hour, minute,
        )?))
    }

    /// Run the task once per month on the given day, UTC hour and minute.
    pub fn monthly_on<T: TaskHandler>(
        day: u32,
        hour: u32,
        minute: u32,
    ) -> Result<CronBuilder<T>, CrontabTimerError> {
        Ok(Self::from_timer(CrontabTimer::monthly_on(
            day, hour, minute,
        )?))
    }

    /// Run the task once per year on the given month, day, UTC hour and minute.
    pub fn yearly_on<T: TaskHandler>(
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    ) -> Result<CronBuilder<T>, CrontabTimerError> {
        Ok(Self::from_timer(CrontabTimer::yearly_on(
            month, day, hour, minute,
        )?))
    }
}

#[cfg(test)]
mod tests;
