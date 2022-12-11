pub enum CrontabPart {
    Minute,
    Hours,
    Days,
    Months,
    DaysOfWeek,
}

impl CrontabPart {
    pub fn boundaries(&self) -> (u8, u8) {
        match self {
            CrontabPart::Minute => (0, 59),
            CrontabPart::Hours => (0, 23),
            CrontabPart::Days => (0, 31),
            CrontabPart::Months => (1, 12),
            CrontabPart::DaysOfWeek => (1, 7),
        }
    }
}

pub enum CrontabTimerValue {
    Number(u8),
    Range(u8, u8),
    Wildcard(Option<u8>),
}
