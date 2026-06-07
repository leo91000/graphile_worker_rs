use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlTextAreaElement};

pub(super) fn textarea_value(event: &Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<HtmlTextAreaElement>().ok())
        .map(|target| target.value())
        .unwrap_or_default()
}

pub(super) fn optional_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(super) fn optional_i16(value: String) -> Option<i16> {
    value.trim().parse().ok()
}

pub(super) fn optional_csv(value: String) -> Option<Vec<String>> {
    let values = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

pub(super) fn datetime_local_to_utc(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let format = if value.matches(':').count() >= 2 {
        "%Y-%m-%dT%H:%M:%S"
    } else {
        "%Y-%m-%dT%H:%M"
    };
    NaiveDateTime::parse_from_str(value, format)
        .ok()
        .and_then(|datetime| {
            let js_local = js_sys::Date::new(&JsValue::from_str(
                &datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
            ));
            let offset_minutes = js_local.get_timezone_offset();
            if !offset_minutes.is_finite() {
                return None;
            }
            Some(DateTime::<Utc>::from_naive_utc_and_offset(
                datetime + Duration::minutes(offset_minutes as i64),
                Utc,
            ))
        })
}
