#![allow(clippy::non_canonical_partial_ord_impl)]

use chrono::{prelude::*, Weekday};
use getset::Getters;
use std::fmt;
use thiserror::Error;

/// A crontab defines a task to be executed at a specific time(s)
#[derive(Debug, PartialEq, Eq, Clone, Getters, Default)]
#[getset(get = "pub")]
pub struct Crontab {
    pub timer: CrontabTimer,
    pub task_identifier: String,
    pub options: CrontabOptions,
    pub payload: Option<serde_json::Value>,
}

/// Crontab time fields used for validating typed schedule construction.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CrontabField {
    Minute,
    Hour,
    Day,
    Month,
}

impl CrontabField {
    const fn bounds(self) -> (u32, u32) {
        match self {
            CrontabField::Minute => (0, 59),
            CrontabField::Hour => (0, 23),
            CrontabField::Day => (1, 31),
            CrontabField::Month => (1, 12),
        }
    }
}

impl fmt::Display for CrontabField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = match self {
            CrontabField::Minute => "minute",
            CrontabField::Hour => "hour",
            CrontabField::Day => "day",
            CrontabField::Month => "month",
        };
        f.write_str(field)
    }
}

/// Error returned when typed schedule constructors receive invalid values.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum CrontabTimerError {
    #[error("{field} value {value} is out of range; expected {min}..={max}")]
    ValueOutOfRange {
        field: CrontabField,
        value: u32,
        min: u32,
        max: u32,
    },
    #[error("{field} range starts at {start} but ends at {end}")]
    RangeOutOfOrder {
        field: CrontabField,
        start: u32,
        end: u32,
    },
    #[error("{field} step must be greater than zero")]
    ZeroStep { field: CrontabField },
}

pub type Result<T> = core::result::Result<T, CrontabTimerError>;

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
    pub const fn new(w: u32, d: u32, h: u32, m: u32, s: u32) -> Self {
        Self { w, d, h, m, s }
    }

    /// Construct a crontab fill from seconds.
    pub const fn seconds(seconds: u32) -> Self {
        Self::new(0, 0, 0, 0, seconds)
    }

    /// Construct a crontab fill from minutes.
    pub const fn minutes(minutes: u32) -> Self {
        Self::new(0, 0, 0, minutes, 0)
    }

    /// Construct a crontab fill from hours.
    pub const fn hours(hours: u32) -> Self {
        Self::new(0, 0, hours, 0, 0)
    }

    /// Construct a crontab fill from days.
    pub const fn days(days: u32) -> Self {
        Self::new(0, days, 0, 0, 0)
    }

    /// Construct a crontab fill from weeks.
    pub const fn weeks(weeks: u32) -> Self {
        Self::new(weeks, 0, 0, 0, 0)
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
    /// Run every minute.
    pub fn every_minute() -> Self {
        Self::default()
    }

    /// Run every `step` minutes.
    pub fn every_n_minutes(step: u32) -> Result<Self> {
        Ok(Self {
            minutes: vec![CrontabValue::step(CrontabField::Minute, step)?],
            ..Default::default()
        })
    }

    /// Run once per hour at the given minute.
    pub fn hourly_at(minute: u32) -> Result<Self> {
        Ok(Self {
            minutes: vec![CrontabValue::number(CrontabField::Minute, minute)?],
            ..Default::default()
        })
    }

    /// Run once per day at the given UTC hour and minute.
    pub fn daily_at(hour: u32, minute: u32) -> Result<Self> {
        Ok(Self {
            minutes: vec![CrontabValue::number(CrontabField::Minute, minute)?],
            hours: vec![CrontabValue::number(CrontabField::Hour, hour)?],
            ..Default::default()
        })
    }

    /// Run once per week on the given weekday, UTC hour and minute.
    pub fn weekly_on(weekday: Weekday, hour: u32, minute: u32) -> Result<Self> {
        Ok(Self {
            minutes: vec![CrontabValue::number(CrontabField::Minute, minute)?],
            hours: vec![CrontabValue::number(CrontabField::Hour, hour)?],
            dows: vec![CrontabValue::Number(weekday.number_from_monday())],
            ..Default::default()
        })
    }

    /// Run once per month on the given day, UTC hour and minute.
    pub fn monthly_on(day: u32, hour: u32, minute: u32) -> Result<Self> {
        Ok(Self {
            minutes: vec![CrontabValue::number(CrontabField::Minute, minute)?],
            hours: vec![CrontabValue::number(CrontabField::Hour, hour)?],
            days: vec![CrontabValue::number(CrontabField::Day, day)?],
            ..Default::default()
        })
    }

    /// Run once per year on the given month, day, UTC hour and minute.
    pub fn yearly_on(month: u32, day: u32, hour: u32, minute: u32) -> Result<Self> {
        Ok(Self {
            minutes: vec![CrontabValue::number(CrontabField::Minute, minute)?],
            hours: vec![CrontabValue::number(CrontabField::Hour, hour)?],
            days: vec![CrontabValue::number(CrontabField::Day, day)?],
            months: vec![CrontabValue::number(CrontabField::Month, month)?],
            ..Default::default()
        })
    }

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

impl CrontabValue {
    fn validate(field: CrontabField, value: u32) -> Result<u32> {
        let (min, max) = field.bounds();
        if value < min || value > max {
            return Err(CrontabTimerError::ValueOutOfRange {
                field,
                value,
                min,
                max,
            });
        }

        Ok(value)
    }

    /// Construct and validate an explicit value for a crontab field.
    pub fn number(field: CrontabField, value: u32) -> Result<Self> {
        Ok(Self::Number(Self::validate(field, value)?))
    }

    /// Construct and validate an inclusive range for a crontab field.
    pub fn range(field: CrontabField, start: u32, end: u32) -> Result<Self> {
        let start = Self::validate(field, start)?;
        let end = Self::validate(field, end)?;

        if start > end {
            return Err(CrontabTimerError::RangeOutOfOrder { field, start, end });
        }

        Ok(Self::Range(start, end))
    }

    /// Construct and validate a step value for a crontab field.
    pub fn step(field: CrontabField, step: u32) -> Result<Self> {
        if step == 0 {
            return Err(CrontabTimerError::ZeroStep { field });
        }

        let (_, max) = field.bounds();
        if step > max {
            return Err(CrontabTimerError::ValueOutOfRange {
                field,
                value: step,
                min: 1,
                max,
            });
        }

        Ok(Self::Step(step))
    }

    /// Check if the value match the crontab value
    /// The step_offset is used to check if the value match a step
    pub(crate) fn match_value(&self, value: u32, step_offset: u32) -> bool {
        match self {
            CrontabValue::Number(n) => &value == n,
            CrontabValue::Range(low, high) => &value >= low && &value <= high,
            CrontabValue::Step(0) => false,
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
    pub fn crontab_fill_constructors() {
        assert_eq!(CrontabFill::seconds(2).to_secs(), 2);
        assert_eq!(CrontabFill::minutes(2).to_secs(), 120);
        assert_eq!(CrontabFill::hours(2).to_secs(), 7200);
        assert_eq!(CrontabFill::days(2).to_secs(), 172800);
        assert_eq!(CrontabFill::weeks(2).to_secs(), 1209600);
    }

    #[test]
    pub fn crontab_timer_constructors_build_expected_values() -> Result<()> {
        assert_eq!(CrontabTimer::every_minute(), CrontabTimer::default());

        assert_eq!(
            CrontabTimer::every_n_minutes(5)?.minutes,
            vec![CrontabValue::Step(5)]
        );

        let daily = CrontabTimer::daily_at(8, 30)?;
        assert_eq!(daily.hours, vec![CrontabValue::Number(8)]);
        assert_eq!(daily.minutes, vec![CrontabValue::Number(30)]);

        let weekly = CrontabTimer::weekly_on(Weekday::Mon, 8, 30)?;
        assert_eq!(weekly.dows, vec![CrontabValue::Number(1)]);

        let monthly = CrontabTimer::monthly_on(15, 8, 30)?;
        assert_eq!(monthly.days, vec![CrontabValue::Number(15)]);

        let yearly = CrontabTimer::yearly_on(12, 15, 8, 30)?;
        assert_eq!(yearly.months, vec![CrontabValue::Number(12)]);

        Ok(())
    }

    #[test]
    pub fn crontab_timer_constructors_validate_values() {
        assert_eq!(
            CrontabTimer::daily_at(24, 0),
            Err(CrontabTimerError::ValueOutOfRange {
                field: CrontabField::Hour,
                value: 24,
                min: 0,
                max: 23,
            })
        );
        assert_eq!(
            CrontabTimer::every_n_minutes(0),
            Err(CrontabTimerError::ZeroStep {
                field: CrontabField::Minute
            })
        );
        assert_eq!(
            CrontabTimer::every_n_minutes(60),
            Err(CrontabTimerError::ValueOutOfRange {
                field: CrontabField::Minute,
                value: 60,
                min: 1,
                max: 59,
            })
        );
        assert_eq!(
            CrontabValue::range(CrontabField::Day, 31, 1),
            Err(CrontabTimerError::RangeOutOfOrder {
                field: CrontabField::Day,
                start: 31,
                end: 1,
            })
        );
    }

    #[test]
    pub fn crontab_value_match_value() {
        assert!(CrontabValue::Number(30).match_value(30, 0));
        assert!(CrontabValue::Range(8, 10).match_value(8, 0));
        assert!(!CrontabValue::Range(8, 10).match_value(7, 0));
        assert!(CrontabValue::Step(4).match_value(5, 1));
        assert!(CrontabValue::Step(9).match_value(9, 0));
        assert!(!CrontabValue::Step(9).match_value(9, 1));
        assert!(!CrontabValue::Step(0).match_value(9, 0));
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
