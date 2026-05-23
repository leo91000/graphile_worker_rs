use std::collections::HashMap;

use chrono::{DateTime, Utc};
use futures::{lock::Mutex, StreamExt};
use serde_json::Value;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{Column, PgPool, Postgres, Row, TypeInfo};

use crate::{
    Database, DatabaseDriver, DbCell, DbError, DbExecutor, DbExecutorArg, DbParams, DbRow,
    DbTransaction, DbValue, Notification, NotificationStream, TransactionDriver,
};

#[derive(Clone, Debug)]
pub struct SqlxDatabase {
    pool: PgPool,
}

impl SqlxDatabase {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl From<PgPool> for SqlxDatabase {
    fn from(pool: PgPool) -> Self {
        Self::new(pool)
    }
}

impl From<PgPool> for Database {
    fn from(pool: PgPool) -> Self {
        Database::new(SqlxDatabase::new(pool))
    }
}

impl From<&PgPool> for Database {
    fn from(pool: &PgPool) -> Self {
        Database::new(SqlxDatabase::new(pool.clone()))
    }
}

impl From<SqlxDatabase> for Database {
    fn from(database: SqlxDatabase) -> Self {
        Database::new(database)
    }
}

impl From<sqlx::Error> for DbError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(database_error) = &error {
            if let Some(code) = database_error.code() {
                return DbError::with_code(error.to_string(), code);
            }
        }

        DbError::new(error.to_string())
    }
}

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

fn bind_params<'q>(
    sql: &'q str,
    params: &DbParams,
) -> sqlx::query::Query<'q, Postgres, PgArguments> {
    let mut query = sqlx::query(sql);
    for value in params.values() {
        query = bind_value(query, value);
    }
    query
}

fn sqlx_row_to_db_row(row: PgRow) -> Result<DbRow, DbError> {
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

impl DbExecutor for SqlxDatabase {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            bind_params(sql, &params)
                .execute(&self.pool)
                .await
                .map(|result| result.rows_affected())
                .map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = bind_params(sql, &params).fetch_all(&self.pool).await?;
            rows.into_iter().map(sqlx_row_to_db_row).collect()
        })
    }
}

impl DbExecutor for PgPool {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            bind_params(sql, &params)
                .execute(self)
                .await
                .map(|result| result.rows_affected())
                .map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = bind_params(sql, &params).fetch_all(self).await?;
            rows.into_iter().map(sqlx_row_to_db_row).collect()
        })
    }
}

impl DbExecutorArg for &mut sqlx::Transaction<'_, Postgres> {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            bind_params(sql, &params)
                .execute((**self).as_mut())
                .await
                .map(|result| result.rows_affected())
                .map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = bind_params(sql, &params)
                .fetch_all((**self).as_mut())
                .await?;
            rows.into_iter().map(sqlx_row_to_db_row).collect()
        })
    }
}

impl DbExecutorArg for &mut sqlx::PgConnection {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            bind_params(sql, &params)
                .execute(&mut **self)
                .await
                .map(|result| result.rows_affected())
                .map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = bind_params(sql, &params).fetch_all(&mut **self).await?;
            rows.into_iter().map(sqlx_row_to_db_row).collect()
        })
    }
}

impl DatabaseDriver for SqlxDatabase {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn begin<'a>(&'a self) -> crate::BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async move {
            let tx = self.pool.begin().await?;
            Ok(DbTransaction::new(Box::new(SqlxTransaction {
                tx: Mutex::new(Some(tx)),
            })))
        })
    }

    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> crate::BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async move {
            let mut listener = sqlx::postgres::PgListener::connect_with(&self.pool).await?;
            listener.listen(channel).await?;
            let stream = listener.into_stream().map(|result| {
                result
                    .map(|notification| Notification {
                        channel: notification.channel().to_string(),
                        payload: notification.payload().to_string(),
                    })
                    .map_err(Into::into)
            });
            Ok(Some(Box::pin(stream) as NotificationStream))
        })
    }
}

pub struct SqlxTransaction {
    tx: Mutex<Option<sqlx::Transaction<'static, Postgres>>>,
}

impl DbExecutor for SqlxTransaction {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let mut tx = self.tx.lock().await;
            let tx = tx
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;

            bind_params(sql, &params)
                .execute(tx.as_mut())
                .await
                .map(|result| result.rows_affected())
                .map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let mut tx = self.tx.lock().await;
            let tx = tx
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;

            let rows = bind_params(sql, &params).fetch_all(tx.as_mut()).await?;
            rows.into_iter().map(sqlx_row_to_db_row).collect()
        })
    }
}

impl TransactionDriver for SqlxTransaction {
    fn commit(self: Box<Self>) -> crate::BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async move {
            let mut tx = self.tx.lock().await;
            let tx = tx
                .take()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            tx.commit().await.map_err(Into::into)
        })
    }
}
