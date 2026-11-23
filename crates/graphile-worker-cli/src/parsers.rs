use std::time::Duration;

use graphile_worker_admin_api::parse_rfc3339_utc_timestamp;
use time::OffsetDateTime;

pub(crate) fn parse_duration(value: &str) -> std::result::Result<Duration, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("duration must not be empty".to_string());
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }

    let (number, unit) = value
        .find(|character: char| !character.is_ascii_digit())
        .map(|index| value.split_at(index))
        .ok_or_else(|| format!("invalid duration `{value}`"))?;

    let amount: u64 = number
        .parse()
        .map_err(|_| format!("invalid duration `{value}`"))?;

    let seconds = match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => amount,
        "m" | "min" | "mins" | "minute" | "minutes" => amount * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => amount * 60 * 60,
        unit => return Err(format!("unsupported duration unit `{unit}`")),
    };

    Ok(Duration::from_secs(seconds))
}

pub(crate) fn parse_utc_datetime(value: &str) -> std::result::Result<OffsetDateTime, String> {
    if value.eq_ignore_ascii_case("now") {
        return Ok(OffsetDateTime::now_utc());
    }

    parse_rfc3339_utc_timestamp(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphile_worker_admin_api::format_utc_timestamp;
    use time::UtcOffset;

    #[test]
    fn parses_rfc3339_datetime_as_utc() {
        let parsed =
            parse_utc_datetime("2026-01-02T03:04:05+02:00").expect("datetime should parse");

        assert_eq!(parsed.offset(), UtcOffset::UTC);
        assert_eq!(
            format_utc_timestamp(parsed).unwrap(),
            "2026-01-02T01:04:05Z"
        );
    }

    #[test]
    fn converts_rfc3339_leap_second_to_postgres_instant() {
        let parsed = parse_utc_datetime("2016-12-31T23:59:60.125Z").unwrap();
        assert_eq!(
            format_utc_timestamp(parsed).unwrap(),
            "2017-01-01T00:00:00.125Z"
        );
    }

    #[test]
    fn parses_now_datetime() {
        let before = OffsetDateTime::now_utc();
        let parsed = parse_utc_datetime("now").expect("now should parse");
        let after = OffsetDateTime::now_utc();

        assert!(parsed >= before);
        assert!(parsed <= after);
    }

    #[test]
    fn parses_duration_units() {
        assert_eq!(
            parse_duration("300").expect("seconds"),
            Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration("5m").expect("minutes"),
            Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration("2h").expect("hours"),
            Duration::from_secs(7200)
        );
    }
}
