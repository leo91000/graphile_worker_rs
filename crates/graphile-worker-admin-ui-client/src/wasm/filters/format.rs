use chrono::{DateTime, Utc};
use serde_json::Value;

pub(super) fn stringify_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

pub(super) fn short(value: &str, length: usize) -> String {
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

pub(super) fn format_date(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}
