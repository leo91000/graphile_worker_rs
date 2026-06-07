use chrono::{prelude::*, Weekday};
use getset::Getters;

use crate::{CrontabField, CrontabValue, Result};

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
