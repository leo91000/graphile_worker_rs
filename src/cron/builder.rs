use graphile_worker_crontab_types::{Crontab, CrontabFill, CrontabTimer, JobKeyMode};
use graphile_worker_task_handler::TaskHandler;
use serde_json::Value;
use std::marker::PhantomData;

/// Builder for a typed cron entry.
#[derive(Debug, Clone)]
pub struct CronBuilder<T: TaskHandler> {
    crontab: Crontab,
    _task: PhantomData<fn() -> T>,
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
