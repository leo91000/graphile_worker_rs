use getset::Getters;

const SECOND: u32 = 1;
const MINUTE: u32 = 60 * SECOND;
const HOUR: u32 = 60 * MINUTE;
const DAY: u32 = 24 * HOUR;
const WEEK: u32 = 7 * DAY;

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
        Some(self.cmp(other))
    }
}

impl Ord for CrontabFill {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_secs().cmp(&other.to_secs())
    }
}
