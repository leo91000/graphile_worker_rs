use getset::Getters;
use time::PrimitiveDateTime;

use crate::{CrontabOptions, CrontabTimer};

/// A crontab defines a task to be executed at a specific time(s)
#[derive(Debug, PartialEq, Eq, Clone, Getters, Default)]
#[getset(get = "pub")]
pub struct Crontab {
    pub timer: CrontabTimer,
    pub task_identifier: String,
    pub options: CrontabOptions,
    pub payload: Option<serde_json::Value>,
}

impl Crontab {
    /// Construct a crontab from a typed timer and task identifier.
    pub fn new(timer: CrontabTimer, task_identifier: impl Into<String>) -> Self {
        Self {
            timer,
            task_identifier: task_identifier.into(),
            ..Default::default()
        }
    }

    /// Shorcut method to timer : check if the crontab should run at specified date
    ///
    /// ```rust
    /// use graphile_worker_crontab_types::{CrontabValue, CrontabTimer, Crontab};
    /// use time::macros::datetime;
    ///
    /// let crontab = Crontab {
    ///    timer: CrontabTimer {
    ///      minutes: vec![CrontabValue::Number(30)],
    ///      hours: vec![CrontabValue::Range(8, 10)],
    ///      days: vec![CrontabValue::Step(4)],
    ///      ..Default::default()
    ///    },
    ///    task_identifier: "test".to_string(),
    ///    ..Default::default()
    /// };
    /// assert!(crontab.should_run_at(&datetime!(2012-12-17 08:30:12)));
    /// assert!(crontab.should_run_at(&datetime!(2015-02-05 09:30:00)));
    /// assert!(crontab.should_run_at(&datetime!(1998-10-13 10:30:59)));
    /// ```
    pub fn should_run_at(&self, at: &PrimitiveDateTime) -> bool {
        self.timer().should_run_at(at)
    }

    /// Get the identifier of the crontab
    /// If the id option is specified, it will be used, otherwise the task identifier will be used
    pub fn identifier(&self) -> &str {
        self.options
            .id
            .as_deref()
            .unwrap_or(self.task_identifier.as_str())
    }
}
