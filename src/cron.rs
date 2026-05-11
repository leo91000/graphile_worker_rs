use chrono::Weekday;
use graphile_worker_crontab_types::{
    Crontab, CrontabFill, CrontabTimer, CrontabTimerError, JobKeyMode,
};
use graphile_worker_task_handler::TaskHandler;
use serde_json::Value;
use std::marker::PhantomData;

/// Namespace for typed cron schedule builders.
///
/// `Cron` creates [`Crontab`] values from Rust types instead of crontab strings.
/// The task identifier is taken from `T::IDENTIFIER`, so it stays aligned with
/// the registered [`TaskHandler`].
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

/// Builder for a typed cron entry.
#[derive(Debug, Clone)]
pub struct CronBuilder<T: TaskHandler> {
    crontab: Crontab,
    _task: PhantomData<fn() -> T>,
}

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

impl<T: TaskHandler> CronBuilder<T> {
    /// Build a typed cron entry from a custom timer.
    pub fn new(timer: CrontabTimer) -> Self {
        Self {
            crontab: Crontab::new(timer, T::IDENTIFIER),
            _task: PhantomData,
        }
    }

    /// Set a stable identifier for this cron entry.
    ///
    /// Use this when more than one schedule targets the same task.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.crontab.options.id = Some(id.into());
        self
    }

    /// Backfill missed executions for the given duration.
    pub fn fill(mut self, fill: CrontabFill) -> Self {
        self.crontab.options.fill = Some(fill);
        self
    }

    /// Override the maximum number of attempts for jobs created by this cron.
    pub fn max_attempts(mut self, max_attempts: u16) -> Self {
        self.crontab.options.max = Some(max_attempts);
        self
    }

    /// Add jobs created by this cron to a named queue.
    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.crontab.options.queue = Some(queue.into());
        self
    }

    /// Override the priority for jobs created by this cron.
    pub fn priority(mut self, priority: i16) -> Self {
        self.crontab.options.priority = Some(priority);
        self
    }

    /// Set a job key for deduplication.
    pub fn job_key(mut self, job_key: impl Into<String>) -> Self {
        self.crontab.options.job_key = Some(job_key.into());
        self
    }

    /// Set the behavior for an existing job with the same job key.
    pub fn job_key_mode(mut self, job_key_mode: JobKeyMode) -> Self {
        self.crontab.options.job_key_mode = Some(job_key_mode);
        self
    }

    /// Serialize a typed task payload for jobs created by this cron.
    pub fn payload(mut self, payload: T) -> Result<Self, serde_json::Error> {
        self.crontab.payload = Some(serde_json::to_value(payload)?);
        Ok(self)
    }

    /// Set a pre-built JSON payload for jobs created by this cron.
    pub fn payload_value(mut self, payload: impl Into<Value>) -> Self {
        self.crontab.payload = Some(payload.into());
        self
    }

    /// Finish the builder and return the lower-level crontab value.
    pub fn build(self) -> Crontab {
        self.crontab
    }
}

impl<T: TaskHandler> From<CronBuilder<T>> for Crontab {
    fn from(builder: CronBuilder<T>) -> Self {
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphile_worker_ctx::WorkerContext;
    use graphile_worker_task_handler::IntoTaskHandlerResult;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Deserialize, Serialize)]
    struct SendDigest {
        message: String,
    }

    impl TaskHandler for SendDigest {
        const IDENTIFIER: &'static str = "send_digest";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
    }

    #[test]
    fn cron_builder_uses_task_identifier_and_options() {
        let crontab = Cron::daily_at::<SendDigest>(8, 30)
            .unwrap()
            .id("daily_digest")
            .fill(CrontabFill::hours(2))
            .max_attempts(3)
            .queue("mail")
            .priority(-1)
            .job_key("daily_digest")
            .job_key_mode(JobKeyMode::PreserveRunAt)
            .payload(SendDigest {
                message: "hello".to_string(),
            })
            .unwrap()
            .build();

        assert_eq!(crontab.task_identifier().as_str(), "send_digest");
        assert_eq!(crontab.identifier(), "daily_digest");
        assert_eq!(
            crontab.timer().hours(),
            &vec![graphile_worker_crontab_types::CrontabValue::Number(8)]
        );
        assert_eq!(
            crontab.timer().minutes(),
            &vec![graphile_worker_crontab_types::CrontabValue::Number(30)]
        );
        assert_eq!(crontab.options().fill(), &Some(CrontabFill::hours(2)));
        assert_eq!(crontab.options().max(), &Some(3));
        assert_eq!(crontab.options().queue(), &Some("mail".to_string()));
        assert_eq!(crontab.options().priority(), &Some(-1));
        assert_eq!(
            crontab.options().job_key(),
            &Some("daily_digest".to_string())
        );
        assert_eq!(
            crontab.options().job_key_mode(),
            &Some(JobKeyMode::PreserveRunAt)
        );
        assert_eq!(crontab.payload(), &Some(json!({ "message": "hello" })));
    }

    #[test]
    fn cron_builder_converts_into_crontab() {
        let crontab: Crontab = Cron::every_minute::<SendDigest>().into();

        assert_eq!(crontab.task_identifier().as_str(), "send_digest");
        assert_eq!(crontab.timer(), &CrontabTimer::every_minute());
    }
}
