use serde_json::Value;
use time::{macros::format_description, OffsetDateTime, UtcOffset};

pub(in crate::wasm) fn stringify_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

pub(in crate::wasm) fn short(value: &str, length: usize) -> String {
    if value.chars().count() <= length {
        return value.to_string();
    }
    let mut output = value
        .chars()
        .take(length.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

pub(in crate::wasm) fn format_date(value: OffsetDateTime) -> String {
    value
        .to_offset(UtcOffset::UTC)
        .format(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second] UTC"
        ))
        .unwrap_or_else(|_| value.to_string())
}
