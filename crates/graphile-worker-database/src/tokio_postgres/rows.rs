use std::collections::HashMap;

use ::tokio_postgres::types::Type;
use ::tokio_postgres::Row;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{DbCell, DbError, DbRow};

pub(super) fn tokio_row_to_db_row(row: Row) -> Result<DbRow, DbError> {
    let mut cells = HashMap::with_capacity(row.columns().len());

    for (index, column) in row.columns().iter().enumerate() {
        let name = column.name().to_string();
        let cell = match *column.type_() {
            Type::BOOL => row
                .try_get::<usize, Option<bool>>(index)?
                .map(DbCell::Bool)
                .unwrap_or(DbCell::Null),
            Type::INT2 => row
                .try_get::<usize, Option<i16>>(index)?
                .map(DbCell::I16)
                .unwrap_or(DbCell::Null),
            Type::INT4 => row
                .try_get::<usize, Option<i32>>(index)?
                .map(DbCell::I32)
                .unwrap_or(DbCell::Null),
            Type::INT8 => row
                .try_get::<usize, Option<i64>>(index)?
                .map(DbCell::I64)
                .unwrap_or(DbCell::Null),
            Type::JSON | Type::JSONB => row
                .try_get::<usize, Option<Value>>(index)?
                .map(DbCell::Json)
                .unwrap_or(DbCell::Null),
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => row
                .try_get::<usize, Option<String>>(index)?
                .map(DbCell::Text)
                .unwrap_or(DbCell::Null),
            Type::TIMESTAMPTZ => row
                .try_get::<usize, Option<DateTime<Utc>>>(index)?
                .map(DbCell::TimestampTz)
                .unwrap_or(DbCell::Null),
            ref other => {
                return Err(DbError::new(format!(
                    "unsupported PostgreSQL result type `{other}` for column `{name}`"
                )));
            }
        };
        cells.insert(name, cell);
    }

    Ok(DbRow::new(cells))
}
