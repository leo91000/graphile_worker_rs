use std::time::Duration;

use chrono::{DateTime, Utc};

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

pub(crate) fn parse_utc_datetime(value: &str) -> std::result::Result<DateTime<Utc>, String> {
    if value.eq_ignore_ascii_case("now") {
        return Ok(Utc::now());
    }

    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_datetime_as_utc() {
        let parsed =
            parse_utc_datetime("2026-01-02T03:04:05+02:00").expect("datetime should parse");

        assert_eq!(parsed.to_rfc3339(), "2026-01-02T01:04:05+00:00");
    }

    #[test]
    fn parses_now_datetime() {
        let before = Utc::now();
        let parsed = parse_utc_datetime("now").expect("now should parse");
        let after = Utc::now();

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
