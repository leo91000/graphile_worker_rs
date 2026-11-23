use serde_json::Value;
use time::OffsetDateTime;

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
    TimestampTz(OffsetDateTime),
    TimestampTzOpt(Option<OffsetDateTime>),
}

#[cfg(any(feature = "driver-sqlx", feature = "driver-tokio-postgres"))]
pub(crate) fn timestamp_in_utc(value: OffsetDateTime) -> Result<OffsetDateTime, crate::DbError> {
    const POSTGRES_EPOCH: OffsetDateTime = time::macros::datetime!(2000-01-01 00:00 UTC);

    let value = value
        .checked_to_offset(time::UtcOffset::UTC)
        .ok_or_else(|| crate::DbError::new("timestamp is outside the supported UTC range"))?;
    let microseconds = (value - POSTGRES_EPOCH).whole_microseconds();
    if matches!(i64::try_from(microseconds), Ok(i64::MIN | i64::MAX)) {
        return Err(crate::DbError::new(
            "PostgreSQL timestamp infinity is unsupported",
        ));
    }
    Ok(value)
}
