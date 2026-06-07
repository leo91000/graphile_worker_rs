use crate::{CrontabField, CrontabTimerError, Result};

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
