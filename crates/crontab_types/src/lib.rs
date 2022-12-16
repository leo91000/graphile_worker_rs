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

#[derive(Debug, PartialEq, Eq)]
pub enum CrontabValue {
    Number(u8),
    Range(u8, u8),
    Step(u8),
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
    pub s: u32,
    pub m: u32,
    pub h: u32,
    pub d: u32,
    pub w: u32,
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

impl CrontabTimer {
    pub fn should_run_at(&self, at: &DateTime<Utc>) -> bool {
        self.minutes()
            .iter()
            .any(|v| v.match_value(&(at.minute() as u8)))
            && self
                .hours()
                .iter()
                .any(|v| v.match_value(&(at.hour() as u8)))
            && self.days().iter().any(|v| v.match_value(&(at.day() as u8)))
            && self
                .months()
                .iter()
                .any(|v| v.match_value(&(at.month() as u8)))
            && self
                .dows()
                .iter()
                .any(|v| v.match_value(&(at.weekday().num_days_from_monday() as u8)))
    }
}

impl CrontabValue {
    fn match_value(&self, value: &u8) -> bool {
        match self {
            CrontabValue::Number(n) => value == n,
            CrontabValue::Range(low, high) => value >= low && value <= high,
            CrontabValue::Step(n) => n % value == 0,
            CrontabValue::Any => true,
        }
    }
}
