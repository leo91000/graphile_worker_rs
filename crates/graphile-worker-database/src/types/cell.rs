use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug)]
pub enum DbCell {
    Null,
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    Json(Value),
    Text(String),
    TimestampTz(DateTime<Utc>),
}
