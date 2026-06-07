use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug)]
pub enum DbValue {
    Bool(bool),
    BoolOpt(Option<bool>),
    I16(i16),
    I16Opt(Option<i16>),
    I32(i32),
    I32Opt(Option<i32>),
    I64(i64),
    I64Opt(Option<i64>),
    Json(Value),
    JsonOpt(Option<Value>),
    Text(String),
    TextOpt(Option<String>),
    TextArray(Vec<String>),
    TextArrayOpt(Option<Vec<String>>),
    I32Array(Vec<i32>),
    I64Array(Vec<i64>),
    TimestampTz(DateTime<Utc>),
    TimestampTzOpt(Option<DateTime<Utc>>),
}
