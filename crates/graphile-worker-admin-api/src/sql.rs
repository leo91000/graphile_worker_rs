use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{FromRow, Postgres};

pub(crate) fn safe_query_as<T>(sql: &str) -> sqlx::query::QueryAs<'_, Postgres, T, PgArguments>
where
    T: for<'row> FromRow<'row, PgRow> + Send + Unpin,
{
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
}
