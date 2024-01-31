#![allow(clippy::non_canonical_partial_ord_impl)]

use chrono::prelude::*;
use getset::Getters;

/// A crontab defines a task to be executed at a specific time(s)
#[derive(Debug, PartialEq, Eq, Clone, Getters, Default)]
#[getset(get = "pub")]
pub struct Crontab {
    pub timer: CrontabTimer,
    pub task_identifier: String,
    pub options: CrontabOptions,
    pub payload: Option<serde_json::Value>,
}

/// A crontab value can be a number, a range, a step or any value
/// It is used to represent a crontab value for a specific field (hour, day, month, etc.)
/// When specifying a specific number, range or step, it should be valid for the field (e.g. 0-59 for minutes, 0-23 for hours, etc.)
#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum CrontabValue {
    Number(u32),
    Range(u32, u32),
    Step(u32),
    #[default]
    Any,
}

/// A crontab timer is a set of crontab values for each field (minutes, hours, days, months, days of week)
#[derive(Debug, PartialEq, Eq, Clone, Getters)]
#[getset(get = "pub")]
pub struct CrontabTimer {
    pub minutes: Vec<CrontabValue>,
    pub hours: Vec<CrontabValue>,
    pub days: Vec<CrontabValue>,
    pub months: Vec<CrontabValue>,
    /// Days of week
    pub dows: Vec<CrontabValue>,
}

/// A crontab fill represents how long a crontab should be backfilled
/// For instance the server is down for 1 hour, the task should be backfilled for 1 hour
#[derive(Debug, PartialEq, Eq, Getters, Clone)]
#[getset(get = "pub")]
pub struct CrontabFill {
    pub w: u32,
    pub d: u32,
    pub h: u32,
    pub m: u32,
    pub s: u32,
}

/// Behavior when an existing job with the same job key is found is controlled by this setting
#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq, Clone)]
pub enum JobKeyMode {
    /// Overwrites the unlocked job with the new values. This is primarily useful for rescheduling, updating, or debouncing
    /// (delaying execution until there have been no events for at least a certain time period).
    /// Locked jobs will cause a new job to be scheduled instead.
    #[serde(rename = "replace")]
    Replace,
    /// overwrites the unlocked job with the new values, but preserves run_at.
    /// This is primarily useful for throttling (executing at most once over a given time period).
    /// Locked jobs will cause a new job to be scheduled instead.
    #[serde(rename = "preserve_run_at")]
    PreserveRunAt,
}

/// Crontab options
#[derive(Debug, PartialEq, Eq, Default, Getters, Clone)]
#[getset(get = "pub")]
pub struct CrontabOptions {
    /// The ID is a unique alphanumeric case-sensitive identifier starting with a letter
    /// Specify an identifier for this crontab entry;
    /// By default this will use the task identifier,
    /// but if you want more than one schedule for the same task (e.g. with different payload, or different times)
    /// then you will need to supply a unique identifier explicitly.
    pub id: Option<String>,
    /// Backfill any entries from the last time period,
    /// for example if the worker was not running
    /// when they were due to be executed (by default, no backfilling).
    pub fill: Option<CrontabFill>,
    /// Override the max_attempts of the job (the max number of retries before giving up).
    pub max: Option<u16>,
    /// Add the job to a named queue so it executes serially with other jobs in the same queue.
    pub queue: Option<String>,
    /// Override the priority of the job (affects the order in which it is executed).
    pub priority: Option<i16>,
    /// Replace/update the existing job with this key, if present.
    pub job_key: Option<String>,
    /// If jobKey is specified, affects what it does.
    pub job_key_mode: Option<JobKeyMode>,
}

const SECOND: u32 = 1;
const MINUTE: u32 = 60 * SECOND;
const HOUR: u32 = 60 * MINUTE;
const DAY: u32 = 24 * HOUR;
const WEEK: u32 = 7 * DAY;

impl CrontabFill {
    /// Construct a crontab fill from a week, a day, an hour, a minute and a second
    pub fn new(w: u32, d: u32, h: u32, m: u32, s: u32) -> Self {
        Self { w, d, h, m, s }
    }

    /// Convert a crontab fill to a number of seconds
    ///
    /// ```rust
    /// use graphile_worker_crontab_types::CrontabFill;
    ///
    /// let fill = CrontabFill::new(0, 0, 0, 0, 0);
    /// assert_eq!(0, fill.to_secs());
    /// let fill = CrontabFill::new(0, 0, 0, 0, 1);
    /// assert_eq!(1, fill.to_secs());
    /// let fill = CrontabFill::new(1, 30, 28, 350, 2);
    /// assert_eq!(3318602, fill.to_secs());
    /// ```
    pub fn to_secs(&self) -> u32 {
        self.w * WEEK + self.d * DAY + self.h * HOUR + self.m * MINUTE + self.s * SECOND
    }
}

impl PartialOrd for CrontabFill {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.to_secs().partial_cmp(&other.to_secs())
    }
}

impl Ord for CrontabFill {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_secs().cmp(&other.to_secs())
    }
}

impl Default for CrontabTimer {
    fn default() -> Self {
        Self {
            minutes: vec![CrontabValue::default()],
            hours: vec![CrontabValue::default()],
            days: vec![CrontabValue::default()],
            months: vec![CrontabValue::default()],
            dows: vec![CrontabValue::default()],
        }
    }
}

impl CrontabTimer {
    /// Check if the timer should run at specifed date
    ///
    /// ```rust
    /// use graphile_worker_crontab_types::{CrontabValue, CrontabTimer};
    ///
    /// let crontab_timer = CrontabTimer {
    ///     minutes: vec![CrontabValue::Number(30)],
    ///     hours: vec![CrontabValue::Range(8, 10)],
    ///     days: vec![CrontabValue::Step(4)],
    ///     ..Default::default()
    /// };
    /// assert!(crontab_timer.should_run_at(&"2012-12-17T08:30:12".parse().unwrap()));
    /// assert!(crontab_timer.should_run_at(&"2015-02-05T09:30:00".parse().unwrap()));
    /// assert!(crontab_timer.should_run_at(&"1998-10-13T10:30:59".parse().unwrap()));
    ///
    /// assert!(!crontab_timer.should_run_at(&"2012-12-17T11:30:59".parse().unwrap()));
    /// assert!(!crontab_timer.should_run_at(&"2015-02-05T09:31:00".parse().unwrap()));
    /// ```
    pub fn should_run_at(&self, at: &NaiveDateTime) -> bool {
        self.minutes().iter().any(|v| v.match_value(at.minute(), 0))
            && self.hours().iter().any(|v| v.match_value(at.hour(), 0))
            && self.days().iter().any(|v| v.match_value(at.day(), 1))
            && self.months().iter().any(|v| v.match_value(at.month(), 1))
            && self
                .dows()
                .iter()
                .any(|v| v.match_value(at.weekday().number_from_monday(), 1))
    }
}

impl Crontab {
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

impl CrontabValue {
    /// Check if the value match the crontab value
    /// The step_offset is used to check if the value match a step
    pub(crate) fn match_value(&self, value: u32, step_offset: u32) -> bool {
        match self {
            CrontabValue::Number(n) => &value == n,
            CrontabValue::Range(low, high) => &value >= low && &value <= high,
            CrontabValue::Step(n) => (value % n) == step_offset,
            CrontabValue::Any => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    pub fn crontab_timer_should_run_at() -> Result<()> {
        let crontab_timer = CrontabTimer {
            minutes: vec![CrontabValue::Number(30)],
            hours: vec![CrontabValue::Range(8, 10)],
            days: vec![CrontabValue::Step(4)],
            ..Default::default()
        };

        assert!(crontab_timer.should_run_at(&"2012-12-17T08:30:12".parse()?));
        assert!(crontab_timer.should_run_at(&"2015-02-05T09:30:00".parse()?));
        assert!(crontab_timer.should_run_at(&"1998-10-13T10:30:59".parse()?));

        assert!(!crontab_timer.should_run_at(&"2012-12-17T11:30:59".parse()?));
        assert!(!crontab_timer.should_run_at(&"2015-02-05T09:31:00".parse()?));
        assert!(!crontab_timer.should_run_at(&"2012-12-13T08:29:12".parse()?));
        assert!(!crontab_timer.should_run_at(&"1998-10-04T10:30:59".parse()?));

        Ok(())
    }

    #[test]
    pub fn crontab_fill_ordering() {
        let biggest_fill = CrontabFill::new(1, 30, 0, 0, 0);
        let bigger_fill = CrontabFill::new(0, 32, 0, 0, 0);
        let lower_fill = CrontabFill::new(0, 0, 432, 0, 0);
        let lowest_fill = CrontabFill::new(2, 0, 0, 0, 0);

        let mut fills = vec![&lower_fill, &bigger_fill, &biggest_fill, &lowest_fill];
        fills.sort();
        assert_eq!(
            vec![&lowest_fill, &lower_fill, &bigger_fill, &biggest_fill],
            fills
        );
    }

    #[test]
    pub fn crontab_fill_to_secs() {
        let fill = CrontabFill::new(0, 0, 0, 0, 0);
        assert_eq!(0, fill.to_secs());
        let fill = CrontabFill::new(0, 0, 0, 0, 1);
        assert_eq!(1, fill.to_secs());
        let fill = CrontabFill::new(1, 30, 28, 350, 2);
        assert_eq!(3318602, fill.to_secs());
    }

    #[test]
    pub fn crontab_value_match_value() {
        assert!(CrontabValue::Number(30).match_value(30, 0));
        assert!(CrontabValue::Range(8, 10).match_value(8, 0));
        assert!(!CrontabValue::Range(8, 10).match_value(7, 0));
        assert!(CrontabValue::Step(4).match_value(5, 1));
        assert!(CrontabValue::Step(9).match_value(9, 0));
        assert!(!CrontabValue::Step(9).match_value(9, 1));
        assert!(CrontabValue::Any.match_value(16, 0));
    }

    #[test]
    pub fn crontab_should_run_at() -> Result<()> {
        let crontab = Crontab {
            timer: CrontabTimer {
                minutes: vec![CrontabValue::Number(30)],
                hours: vec![CrontabValue::Range(8, 10)],
                days: vec![CrontabValue::Step(4)],
                ..Default::default()
            },
            task_identifier: "test".to_string(),
            ..Default::default()
        };
        assert!(crontab.should_run_at(&"2012-12-17T08:30:12".parse()?));
        assert!(crontab.should_run_at(&"2015-02-05T09:30:00".parse()?));
        assert!(crontab.should_run_at(&"1998-10-13T10:30:59".parse()?));

        assert!(!crontab.should_run_at(&"2012-12-17T11:30:59".parse()?));
        assert!(!crontab.should_run_at(&"2015-02-05T09:31:00".parse()?));
        assert!(!crontab.should_run_at(&"2012-12-13T08:29:12".parse()?));
        assert!(!crontab.should_run_at(&"1998-10-04T10:30:59".parse()?));

        Ok(())
    }
}
