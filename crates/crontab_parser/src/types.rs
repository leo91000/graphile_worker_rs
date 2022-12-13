use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub enum CrontabValue {
    Number(u8),
    Range(u8, u8),
    Step(u8),
    Any,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CrontabTimer {
    pub minutes: Vec<CrontabValue>,
    pub hours: Vec<CrontabValue>,
    pub days: Vec<CrontabValue>,
    pub months: Vec<CrontabValue>,
    pub dows: Vec<CrontabValue>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CrontabFill {
    pub s: u32,
    pub m: u32,
    pub h: u32,
    pub d: u32,
    pub w: u32,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct CrontabOptions {
    pub id: Option<String>,
    pub fill: Option<CrontabFill>,
    pub max: Option<u16>,
    pub queue: Option<String>,
    pub priority: Option<i16>,
}
