use chrono::prelude::*;
use getset::Getters;

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
    /// use chrono::prelude::*;
    /// use std::str::FromStr;
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
    /// assert!(crontab.should_run_at(&"2012-12-17T08:30:12".parse().unwrap()));
    /// assert!(crontab.should_run_at(&"2015-02-05T09:30:00".parse().unwrap()));
    /// assert!(crontab.should_run_at(&"1998-10-13T10:30:59".parse().unwrap()));
    /// ```
    pub fn should_run_at(&self, at: &NaiveDateTime) -> bool {
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
