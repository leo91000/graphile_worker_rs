use std::fmt;

use thiserror::Error;

/// Crontab time fields used for validating typed schedule construction.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CrontabField {
    Minute,
    Hour,
    Day,
    Month,
}

impl CrontabField {
    pub(crate) const fn bounds(self) -> (u32, u32) {
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
