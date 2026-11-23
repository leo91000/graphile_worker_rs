use sqlx::{
    error::BoxDynError,
    postgres::{PgTypeInfo, PgValueFormat, PgValueRef},
    Decode, Postgres, Type,
};
use time::{macros::format_description, OffsetDateTime};

use crate::types::timestamp_in_utc;

/// Decode PostgreSQL timestamps for SQLx row derives using either protocol.
#[doc(hidden)]
pub struct SqlxTimestamp<T>(T);

impl<T: Type<Postgres>> Type<Postgres> for SqlxTimestamp<T> {
    fn type_info() -> PgTypeInfo {
        T::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        T::compatible(ty)
    }
}

impl<'r> Decode<'r, Postgres> for SqlxTimestamp<OffsetDateTime> {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        if value.format() == PgValueFormat::Binary {
            return Ok(Self(timestamp_in_utc(OffsetDateTime::decode(value)?)?));
        }

        // PostgreSQL's ISO text format allows a missing fraction and a whole-hour offset.
        let format = format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second][optional [.[subsecond]]][offset_hour sign:mandatory][optional [:[offset_minute][optional [:[offset_second]]]]]"
        );
        Ok(Self(timestamp_in_utc(OffsetDateTime::parse(
            value.as_str()?,
            format,
        )?)?))
    }
}

impl<'r> Decode<'r, Postgres> for SqlxTimestamp<Option<OffsetDateTime>> {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        Option::<SqlxTimestamp<OffsetDateTime>>::decode(value)
            .map(|value| Self(value.map(|timestamp| timestamp.0)))
    }
}

impl<T> SqlxTimestamp<T> {
    pub(super) fn into_inner(self) -> T {
        self.0
    }
}

impl From<SqlxTimestamp<OffsetDateTime>> for OffsetDateTime {
    fn from(value: SqlxTimestamp<OffsetDateTime>) -> Self {
        value.0
    }
}

impl From<SqlxTimestamp<Option<OffsetDateTime>>> for Option<OffsetDateTime> {
    fn from(value: SqlxTimestamp<Option<OffsetDateTime>>) -> Self {
        value.0
    }
}
