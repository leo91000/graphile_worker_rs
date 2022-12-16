use chrono::prelude::*;
use getset::Getters;

#[derive(Debug, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub struct Crontab {
    pub timer: CrontabTimer,
    pub task_identifier: String,
    pub options: CrontabOptions,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum CrontabValue {
    Number(u8),
    Range(u8, u8),
    Step(u8),
    #[default]
    Any,
}

#[derive(Debug, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub struct CrontabTimer {
    pub minutes: Vec<CrontabValue>,
    pub hours: Vec<CrontabValue>,
    pub days: Vec<CrontabValue>,
    pub months: Vec<CrontabValue>,
    /// Days of week
    pub dows: Vec<CrontabValue>,
}

#[derive(Debug, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub struct CrontabFill {
    pub w: u32,
    pub d: u32,
    pub h: u32,
    pub m: u32,
    pub s: u32,
}

#[derive(Debug, PartialEq, Eq, Default, Getters)]
#[getset(get = "pub")]
pub struct CrontabOptions {
    pub id: Option<String>,
    pub fill: Option<CrontabFill>,
    pub max: Option<u16>,
    pub queue: Option<String>,
    pub priority: Option<i16>,
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
    /// use crontab_types::CrontabFill;
    ///
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
    /// use crontab_types::{CrontabValue, CrontabTimer};
    ///
    /// let crontab_timer = CrontabTimer {
    ///     minutes: vec![CrontabValue::Number(30)],
    ///     hours: vec![CrontabValue::Range(8, 10)],
    ///     days: vec![CrontabValue::Step(4)],
    ///     ..Default::default()
    /// };
    /// assert!(crontab_timer.should_run_at(&"2012-12-17T08:30:12".parse().unwrap()));
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

impl CrontabValue {
    fn match_value(&self, value: u32, step_offset: u32) -> bool {
        match self {
            CrontabValue::Number(n) => value == *n as u32,
            CrontabValue::Range(low, high) => value >= *low as u32 && value <= *high as u32,
            CrontabValue::Step(n) => value % *n as u32 == step_offset,
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
}
