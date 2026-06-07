use serde_json::Value;

pub fn default_limit() -> i64 {
    100
}

pub(crate) fn default_payload() -> Value {
    serde_json::json!({})
}
