use std::fmt::Write;

use serde::Serializer;
use time::{error::Format, macros::format_description, OffsetDateTime, UtcOffset};

/// Format a UTC timestamp in the admin wire format, including signed years outside RFC 3339.
pub fn format_utc_timestamp(value: OffsetDateTime) -> Result<String, Format> {
    format_utc(value, "Z")
}

/// Format a UTC timestamp with a numeric offset for CLI and CSV output.
pub fn format_utc_timestamp_with_offset(value: OffsetDateTime) -> Result<String, Format> {
    format_utc(value, "+00:00")
}

fn format_utc(value: OffsetDateTime, suffix: &str) -> Result<String, Format> {
    let value = value
        .checked_to_offset(UtcOffset::UTC)
        .ok_or(Format::InvalidComponent("year"))?;
    let date_time = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
    let mut formatted = value.format(date_time)?;

    let nanos = value.nanosecond();
    if nanos != 0 {
        // Keep the legacy millisecond, microsecond, or nanosecond precision.
        let (fraction, digits) = if nanos.is_multiple_of(1_000_000) {
            (nanos / 1_000_000, 3)
        } else if nanos.is_multiple_of(1_000) {
            (nanos / 1_000, 6)
        } else {
            (nanos, 9)
        };
        write!(formatted, ".{fraction:0digits$}").expect("writing to a String cannot fail");
    }

    formatted.push_str(suffix);
    Ok(formatted)
}

#[doc(hidden)]
pub fn serialize_utc_timestamp<S: Serializer>(
    value: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let value = format_utc_timestamp(*value).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&value)
}
