use std::borrow::Cow;

pub(crate) use graphile_worker_job::serialize_utc_timestamp as serialize;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{
    format_description::{well_known::Rfc3339, BorrowedFormatItem},
    macros::format_description,
    Duration, OffsetDateTime, UtcOffset,
};

const LEGACY_DATE_TIME: &[BorrowedFormatItem<'_>] = format_description!(
    "[year padding:none]-[month padding:none]-[day padding:none][first [T][t][ ]][hour padding:none]:[minute padding:none]:[second padding:none][optional [.[subsecond]]][offset_hour sign:mandatory]:[offset_minute]"
);

/// Parse an RFC 3339 timestamp and normalize it to UTC.
pub fn parse_rfc3339_utc_timestamp(value: &str) -> Result<OffsetDateTime, String> {
    to_utc(parse_rfc3339(value)?)
}

fn parse_rfc3339(value: &str) -> Result<OffsetDateTime, String> {
    if !matches!(value.as_bytes().get(10), Some(b'T' | b't' | b' ')) {
        return Err("expected an RFC 3339 date and time".into());
    }
    // Chrono accepted the Unicode minus sign in offsets.
    let value = match value.find('−') {
        Some(index) if index >= 19 => Cow::Owned(value.replace('−', "-")),
        _ => Cow::Borrowed(value),
    };
    parse_with_leap_second(&value, |value| OffsetDateTime::parse(value, &Rfc3339))
}

fn parse_with_leap_second(
    value: &str,
    parse: impl FnOnce(&str) -> Result<OffsetDateTime, time::error::Parse>,
) -> Result<OffsetDateTime, String> {
    let leap_second = value.match_indices(':').nth(1).and_then(|(colon, _)| {
        let start = colon + 1;
        (value.get(start..start + 2) == Some("60")).then_some(start..start + 2)
    });
    let Some(leap_second) = leap_second else {
        return parse(value).map_err(|error| error.to_string());
    };

    // PostgreSQL stored Chrono leap seconds as the following second plus the original fraction.
    let mut preceding_second = value.to_owned();
    preceding_second.replace_range(leap_second, "59");
    parse(&preceding_second)
        .map_err(|error| error.to_string())?
        .checked_add(Duration::seconds(1))
        .ok_or_else(|| "leap second is outside the supported timestamp range".into())
}

fn to_utc(timestamp: OffsetDateTime) -> Result<OffsetDateTime, String> {
    if timestamp.offset().whole_seconds().unsigned_abs() >= 24 * 60 * 60 {
        return Err("timestamp offset must be less than 24 hours".into());
    }
    timestamp
        .checked_to_offset(UtcOffset::UTC)
        .ok_or_else(|| "timestamp is outside the supported UTC range".into())
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, String> {
    let value = value.trim();
    if let Ok(timestamp) = parse_rfc3339(value) {
        return to_utc(timestamp);
    }

    let value = normalize_legacy_whitespace(value)?;
    let value = normalize_legacy_year(&value);
    let without_zone = value.strip_suffix(['Z', 'z']).or_else(|| {
        let suffix_start = value.len().checked_sub(3)?;
        let (date_time, zone) = value.split_at_checked(suffix_start)?;
        zone.eq_ignore_ascii_case("UTC").then_some(date_time)
    });
    let value = match without_zone {
        Some(date_time) => Cow::Owned(format!("{}+00:00", date_time.trim_end())),
        None => normalize_legacy_offset(&value)?,
    };
    to_utc(parse_with_leap_second(&value, |value| {
        OffsetDateTime::parse(value, LEGACY_DATE_TIME)
    })?)
}

fn normalize_legacy_year(value: &str) -> Cow<'_, str> {
    let Some((year, date)) = value
        .strip_prefix(['+', '-'])
        .and_then(|unsigned| unsigned.split_once('-'))
    else {
        return Cow::Borrowed(value);
    };
    if year.len() <= 6 || !year.starts_with('0') {
        return Cow::Borrowed(value);
    }

    let year = year.trim_start_matches('0');
    let year = if year.is_empty() { "0" } else { year };
    Cow::Owned(format!("{}{year}-{date}", &value[..1]))
}

fn normalize_legacy_whitespace(value: &str) -> Result<Cow<'_, str>, String> {
    if !value.chars().any(char::is_whitespace) {
        return Ok(Cow::Borrowed(value));
    }

    let normalized = (|| {
        let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
        let sign = &value[..value.len() - unsigned.len()];
        let (year, rest) = unsigned.split_once('-')?;
        let (month, day_and_clock) = rest.split_once('-')?;
        let day_and_clock = day_and_clock.trim_start();
        let end_of_day = day_and_clock.find(|ch: char| !ch.is_ascii_digit())?;
        let (day, clock) = day_and_clock.split_at(end_of_day);
        let clock = clock.strip_prefix(['T', 't', ' '])?;
        let (hour, rest) = clock.split_once(':')?;
        let (minute, rest) = rest.split_once(':')?;
        Some(format!(
            "{sign}{}-{}-{day}T{}:{}:{}",
            year.trim_end(),
            month.trim(),
            hour.trim(),
            minute.trim(),
            rest.trim_start(),
        ))
    })();

    normalized
        .map(Cow::Owned)
        .ok_or_else(|| "expected a timestamp date and time".into())
}

fn normalize_legacy_offset(value: &str) -> Result<Cow<'_, str>, String> {
    let offset = value.match_indices(':').nth(1).and_then(|(colon, _)| {
        let seconds = colon + 1;
        value[seconds..]
            .match_indices(['+', '-', '−'])
            .next()
            .map(|(index, sign)| (seconds + index, sign))
    });
    let Some((offset_start, sign)) = offset else {
        return Ok(Cow::Borrowed(value));
    };

    let (hours, minutes) = value[offset_start + sign.len()..]
        .split_at_checked(2)
        .ok_or_else(|| "expected timestamp offset hours and minutes".to_owned())?;
    let minutes = minutes.trim_start_matches(|ch: char| ch == ':' || ch.is_whitespace());
    let sign = if sign == "−" { "-" } else { sign };
    let date_time = value[..offset_start].trim_end();
    Ok(Cow::Owned(format!("{date_time}{sign}{hours}:{minutes}")))
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
