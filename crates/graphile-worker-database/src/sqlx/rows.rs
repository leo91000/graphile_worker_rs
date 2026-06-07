use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, TypeInfo};

use crate::{DbCell, DbError, DbRow};

pub(super) fn sqlx_row_to_db_row(row: PgRow) -> Result<DbRow, DbError> {
    let mut cells = HashMap::with_capacity(row.columns().len());

    for (index, column) in row.columns().iter().enumerate() {
        let name = column.name().to_string();
        let type_name = column.type_info().name();
        let cell = match type_name {
            "BOOL" => row
                .try_get::<Option<bool>, _>(index)?
                .map(DbCell::Bool)
                .unwrap_or(DbCell::Null),
            "INT2" => row
                .try_get::<Option<i16>, _>(index)?
                .map(DbCell::I16)
                .unwrap_or(DbCell::Null),
            "INT4" => row
                .try_get::<Option<i32>, _>(index)?
                .map(DbCell::I32)
                .unwrap_or(DbCell::Null),
            "INT8" => row
                .try_get::<Option<i64>, _>(index)?
                .map(DbCell::I64)
                .unwrap_or(DbCell::Null),
            "JSON" | "JSONB" => row
                .try_get::<Option<Value>, _>(index)?
                .map(DbCell::Json)
                .unwrap_or(DbCell::Null),
            "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" => row
                .try_get::<Option<String>, _>(index)?
                .map(DbCell::Text)
                .unwrap_or(DbCell::Null),
            "TIMESTAMPTZ" => row
                .try_get::<Option<DateTime<Utc>>, _>(index)?
                .map(DbCell::TimestampTz)
                .unwrap_or(DbCell::Null),
            other => {
                return Err(DbError::new(format!(
                    "unsupported PostgreSQL result type `{other}` for column `{name}`"
                )));
            }
        };
        cells.insert(name, cell);
    }

    Ok(DbRow::new(cells))
}
