use graphile_worker_crontab_parser::CrontabParseError;
use graphile_worker_crontab_types::Crontab;

use super::{CronInput, WorkerOptions};

impl WorkerOptions {
    /// Adds cron entries for scheduled jobs.
    ///
    /// This accepts typed cron builders, raw [`Crontab`] values, and crontab
    /// text. Typed inputs return `WorkerOptions` directly; text input returns
    /// `Result<WorkerOptions, CrontabParseError>`.
    ///
    /// # Typed example
    /// ```
    /// # use graphile_worker::{Cron, CrontabFill, WorkerOptions};
    /// # use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Deserialize, Serialize)]
    /// # struct SendDigest;
    /// #
    /// # impl TaskHandler for SendDigest {
    /// #     const IDENTIFIER: &'static str = "send_digest";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
    /// # }
    ///
    /// let options = WorkerOptions::default()
    ///     .define_job::<SendDigest>()
    ///     .with_cron(
    ///         Cron::daily_at::<SendDigest>(8, 0)
    ///             .expect("valid cron schedule")
    ///             .fill(CrontabFill::hours(1)),
    ///     );
    /// ```
    ///
    /// # Crontab text example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let options = WorkerOptions::default()
    ///     .with_cron("0 8 * * * send_digest")?;
    /// # let _ = options;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_cron<C: CronInput>(self, cron: C) -> C::Output {
        cron.append_to(self)
    }

    /// Adds typed cron entries for scheduled jobs.
    pub fn with_crons<I, C>(mut self, crontabs: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Crontab>,
    {
        self.append_crontabs(crontabs.into_iter().map(Into::into).collect());
        self
    }

    /// Adds crontab text entries for scheduled jobs.
    ///
    /// Use [`Self::with_cron`] with a string instead.
    ///
    /// # Arguments
    /// * `input` - A string containing crontab entries
    ///
    /// # Returns
    /// * `Result<Self, CrontabParseError>` - The modified WorkerOptions instance or a parse error
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// // Run the "send_digest" job at 8:00 AM every day.
    /// let options = WorkerOptions::default()
    ///     .with_cron("0 8 * * * send_digest")?;
    /// # Ok(())
    /// # }
    /// ```
    #[deprecated(note = "use WorkerOptions::with_cron(...) instead")]
    pub fn with_crontab(self, input: &str) -> Result<Self, CrontabParseError> {
        self.with_cron(input)
    }
}
