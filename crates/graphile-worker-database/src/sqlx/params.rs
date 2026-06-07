use sqlx::postgres::PgArguments;
use sqlx::{AssertSqlSafe, Postgres};

use crate::{DbParams, DbValue};

fn bind_value<'q>(
    query: sqlx::query::Query<'q, Postgres, PgArguments>,
    value: &DbValue,
) -> sqlx::query::Query<'q, Postgres, PgArguments> {
    match value {
        DbValue::Bool(value) => query.bind(*value),
        DbValue::BoolOpt(value) => query.bind(*value),
        DbValue::I16(value) => query.bind(*value),
        DbValue::I16Opt(value) => query.bind(*value),
        DbValue::I32(value) => query.bind(*value),
        DbValue::I32Opt(value) => query.bind(*value),
        DbValue::I64(value) => query.bind(*value),
        DbValue::I64Opt(value) => query.bind(*value),
        DbValue::Json(value) => query.bind(value.clone()),
        DbValue::JsonOpt(value) => query.bind(value.clone()),
        DbValue::Text(value) => query.bind(value.clone()),
        DbValue::TextOpt(value) => query.bind(value.clone()),
        DbValue::TextArray(value) => query.bind(value.clone()),
        DbValue::TextArrayOpt(value) => query.bind(value.clone()),
        DbValue::I32Array(value) => query.bind(value.clone()),
        DbValue::I64Array(value) => query.bind(value.clone()),
        DbValue::TimestampTz(value) => query.bind(*value),
        DbValue::TimestampTzOpt(value) => query.bind(*value),
    }
}

pub(super) fn bind_params<'q>(
    sql: &'q str,
    params: &DbParams,
) -> sqlx::query::Query<'q, Postgres, PgArguments> {
    let mut query = sqlx::query(AssertSqlSafe(sql));
    for value in params.values() {
        query = bind_value(query, value);
    }
    query
}
