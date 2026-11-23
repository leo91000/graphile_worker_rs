use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{
    error::Format,
    format_description::{well_known::Rfc3339, BorrowedFormatItem},
    macros::format_description,
    Duration, OffsetDateTime, PrimitiveDateTime, UtcOffset,
};

const DATE_TIME: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]");
const DATE_TIME_WITH_OFFSET: &[BorrowedFormatItem<'_>] = format_description!(
    "[year]-[month]-[day][first [T][t][ ]][hour]:[minute]:[second][optional [.[subsecond]]][offset_hour sign:mandatory][optional [:]][offset_minute]"
);

/// Format a UTC timestamp in the admin wire format, including signed years outside RFC 3339.
pub fn format_utc_timestamp(value: OffsetDateTime) -> Result<String, Format> {
    let value = value
        .checked_to_offset(UtcOffset::UTC)
        .ok_or(Format::InvalidComponent("year"))?;
    if (0..=9999).contains(&value.year()) {
        return value.format(&Rfc3339);
    }

    let mut formatted = value.format(DATE_TIME)?;
    if let Some(without_fraction) = formatted.strip_suffix(".0") {
        formatted.truncate(without_fraction.len());
    }
    formatted.push('Z');
    Ok(formatted)
}

/// Parse an RFC 3339 timestamp and normalize it to UTC.
pub fn parse_rfc3339_utc_timestamp(value: &str) -> Result<OffsetDateTime, String> {
    to_utc(parse_rfc3339(value)?)
}

fn parse_rfc3339(value: &str) -> Result<OffsetDateTime, String> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| error.to_string())?;
    if value.as_bytes().get(17..19) != Some(b"60") {
        return Ok(parsed);
    }

    // PostgreSQL stored Chrono leap seconds as the following second plus the original fraction.
    let mut preceding_second = value.to_owned();
    preceding_second.replace_range(17..19, "59");
    OffsetDateTime::parse(&preceding_second, &Rfc3339)
        .map_err(|error| error.to_string())?
        .checked_add(Duration::seconds(1))
        .ok_or_else(|| "leap second is outside the supported timestamp range".into())
}

fn to_utc(timestamp: OffsetDateTime) -> Result<OffsetDateTime, String> {
    timestamp
        .checked_to_offset(UtcOffset::UTC)
        .ok_or_else(|| "timestamp is outside the supported UTC range".into())
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, String> {
    let value = value.trim();
    if let Ok(timestamp) = parse_rfc3339(value) {
        return to_utc(timestamp);
    }

    if let Some(date_time) = value.strip_suffix(['Z', 'z']) {
        return PrimitiveDateTime::parse(date_time, DATE_TIME)
            .map(PrimitiveDateTime::assume_utc)
            .map_err(|error| error.to_string());
    }

    let timestamp =
        OffsetDateTime::parse(value, DATE_TIME_WITH_OFFSET).map_err(|error| error.to_string())?;
    to_utc(timestamp)
}

pub(crate) fn serialize<S: Serializer>(
    value: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let value = format_utc_timestamp(*value).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&value)
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<OffsetDateTime, D::Error> {
    let value = String::deserialize(deserializer)?;
    parse_timestamp(&value).map_err(serde::de::Error::custom)
}

pub(crate) mod option {
    use super::*;

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    struct Timestamp(#[serde(with = "super")] OffsetDateTime);

    pub(crate) fn serialize<S: Serializer>(
        value: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Timestamp).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        Option::<Timestamp>::deserialize(deserializer).map(|value| value.map(|Timestamp(ts)| ts))
    }
}
