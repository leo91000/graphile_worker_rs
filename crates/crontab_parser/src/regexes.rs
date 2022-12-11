use once_cell::sync::Lazy;
use regex::Regex;

// A (non-comment, non-empty) line in the crontab file
/// Separates crontab line into the minute, hour, day of month, month, day of week and command parts.
pub const CRONTAB_LINE_PARTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        "/^([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+(.*)$/",
    )
    .unwrap()
});

/// Just the time expression from CRONTAB_LINE_PARTS
pub const CRONTAB_TIME_PARTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new("/^([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+([0-9*/,-]+)\\s+([0-9*/,-]+)$")
        .unwrap()
});

// Crontab ranges from the minute, hour, day of month, month and day of week parts of the crontab line
/// Matches an explicit numeric value
pub const CRONTAB_NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new("/^([0-9]+)$/").unwrap());

/// Matches a range of numeric values
pub const CRONTAB_RANGE: Lazy<Regex> = Lazy::new(|| Regex::new("/^([0-9]+)-([0-9]+)$/").unwrap());

/// Matches a numeric wildcard, optionally with a divisor
pub const CRONTAB_WILDCARD: Lazy<Regex> =
    Lazy::new(|| Regex::new("/^\\*(?:\\/([0-9]+))?$/").unwrap());

// The command from the crontab line
/// Splits the command from the crontab line into the task, options and payload.
pub const CRONTAB_COMMAND: Lazy<Regex> = Lazy::new(|| {
    Regex::new("/^([_a-zA-Z][_a-zA-Z0-9:_-]*)(?:\\s+\\?([^\\s]+))?(?:\\s+(\\{.*\\}))?$/").unwrap()
});

// Crontab command options
/// Matches the id=UID option, capturing the unique identifier
pub const CRONTAB_OPTIONS_ID: Lazy<Regex> =
    Lazy::new(|| Regex::new("/^([_a-zA-Z][-_a-zA-Z0-9]*)$/").unwrap());

/// Matches the fill=t option, capturing the time phrase
pub const CRONTAB_OPTIONS_BACKFILL: Lazy<Regex> =
    Lazy::new(|| Regex::new("/^((?:[0-9]+[smhdw])+)$/").unwrap());

/// Matches the max=n option, capturing the max executions number
pub const CRONTAB_OPTIONS_MAX: Lazy<Regex> = Lazy::new(|| Regex::new("/^([0-9]+)$/").unwrap());

/// Matches the queue=name option, capturing the queue name
pub const CRONTAB_OPTIONS_QUEUE: Lazy<Regex> =
    Lazy::new(|| Regex::new("/^([-a-zA-Z0-9_:]+)$/").unwrap());

/// Matches the priority=n option, capturing the priority values
pub const CRONTAB_OPTIONS_PRIORITY: Lazy<Regex> =
    Lazy::new(|| Regex::new("/^(-?[0-9]+)$/").unwrap());

/// Matches the quantity and period string at the beginning of a timephrase
pub const TIMEPHRASE_PART: Lazy<Regex> = Lazy::new(|| Regex::new("/^([0-9]+)([smhdw])/").unwrap());
