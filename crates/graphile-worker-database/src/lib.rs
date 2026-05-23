use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use futures::Stream;
use serde_json::Value;
use thiserror::Error;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type NotificationStream = Pin<Box<dyn Stream<Item = Result<Notification, DbError>> + Send>>;

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

#[derive(Clone, Debug, Default)]
pub struct DbParams(Vec<DbValue>);

impl DbParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: DbValue) {
        self.0.push(value);
    }

    pub fn values(&self) -> &[DbValue] {
        &self.0
    }
}

impl From<Vec<DbValue>> for DbParams {
    fn from(value: Vec<DbValue>) -> Self {
        Self(value)
    }
}

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

#[derive(Clone, Debug, Default)]
pub struct DbRow {
    cells: HashMap<String, DbCell>,
}

impl DbRow {
    pub fn new(cells: HashMap<String, DbCell>) -> Self {
        Self { cells }
    }

    pub fn try_get<T: FromDbCell>(&self, name: &str) -> Result<T, DbError> {
        let cell = self.cells.get(name).ok_or_else(|| {
            DbError::new(format!("column `{name}` was not present in query result"))
        })?;
        T::from_cell(name, cell)
    }
}

pub trait FromDbCell: Sized {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError>;
}

fn type_error(name: &str, expected: &str, cell: &DbCell) -> DbError {
    DbError::new(format!(
        "column `{name}` could not be decoded as {expected}; actual value was {cell:?}"
    ))
}

impl FromDbCell for bool {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::Bool(value) => Ok(*value),
            _ => Err(type_error(name, "bool", cell)),
        }
    }
}

impl FromDbCell for i16 {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::I16(value) => Ok(*value),
            _ => Err(type_error(name, "i16", cell)),
        }
    }
}

impl FromDbCell for i32 {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::I32(value) => Ok(*value),
            _ => Err(type_error(name, "i32", cell)),
        }
    }
}

impl FromDbCell for i64 {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::I64(value) => Ok(*value),
            _ => Err(type_error(name, "i64", cell)),
        }
    }
}

impl FromDbCell for String {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::Text(value) => Ok(value.clone()),
            _ => Err(type_error(name, "String", cell)),
        }
    }
}

impl FromDbCell for Value {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::Json(value) => Ok(value.clone()),
            _ => Err(type_error(name, "serde_json::Value", cell)),
        }
    }
}

impl FromDbCell for DateTime<Utc> {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::TimestampTz(value) => Ok(*value),
            _ => Err(type_error(name, "DateTime<Utc>", cell)),
        }
    }
}

impl FromDbCell for DateTime<Local> {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        let value = DateTime::<Utc>::from_cell(name, cell)?;
        Ok(value.with_timezone(&Local))
    }
}

impl<T: FromDbCell> FromDbCell for Option<T> {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        if matches!(cell, DbCell::Null) {
            return Ok(None);
        }

        T::from_cell(name, cell).map(Some)
    }
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub channel: String,
    pub payload: String,
}

#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct DbError {
    message: String,
    code: Option<String>,
}

impl DbError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    pub fn with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some(code.into()),
        }
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
}

pub trait DbExecutor: Send + Sync {
    fn execute<'a>(&'a self, sql: &'a str, params: DbParams)
        -> BoxFuture<'a, Result<u64, DbError>>;

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>>;

    fn fetch_optional<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Option<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = self.fetch_all(sql, params).await?;
            Ok(rows.into_iter().next())
        })
    }

    fn fetch_one<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<DbRow, DbError>> {
        Box::pin(async move {
            self.fetch_optional(sql, params).await?.ok_or_else(|| {
                DbError::new("query returned no rows when exactly one row was expected")
            })
        })
    }
}

pub trait DbExecutorArg: Send {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>>;

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>>;

    fn fetch_optional<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Option<DbRow>, DbError>>
    where
        Self: Send + 'a,
    {
        Box::pin(async move {
            let rows = self.fetch_all(sql, params).await?;
            Ok(rows.into_iter().next())
        })
    }

    fn fetch_one<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<DbRow, DbError>>
    where
        Self: Send + 'a,
    {
        Box::pin(async move {
            self.fetch_optional(sql, params).await?.ok_or_else(|| {
                DbError::new("query returned no rows when exactly one row was expected")
            })
        })
    }
}

impl<T: DbExecutor + ?Sized> DbExecutorArg for &T {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        DbExecutor::execute(*self, sql, params)
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        DbExecutor::fetch_all(*self, sql, params)
    }
}

impl<T: DbExecutorArg + ?Sized> DbExecutorArg for &mut T {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        (**self).execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        (**self).fetch_all(sql, params)
    }
}

pub trait DatabaseDriver: DbExecutor + fmt::Debug + Any {
    fn as_any(&self) -> &dyn Any;

    fn begin<'a>(&'a self) -> BoxFuture<'a, Result<DbTransaction, DbError>>;

    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>>;
}

pub trait TransactionDriver: DbExecutor {
    fn commit(self: Box<Self>) -> BoxFuture<'static, Result<(), DbError>>;
}

#[derive(Clone)]
pub struct Database {
    inner: Arc<dyn DatabaseDriver>,
}

impl Database {
    pub fn new(driver: impl DatabaseDriver + 'static) -> Self {
        Self {
            inner: Arc::new(driver),
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.as_any().downcast_ref()
    }

    pub async fn begin(&self) -> Result<DbTransaction, DbError> {
        self.inner.begin().await
    }

    pub async fn listen(&self, channel: &str) -> Result<Option<NotificationStream>, DbError> {
        self.inner.listen(channel).await
    }
}

impl From<&Database> for Database {
    fn from(database: &Database) -> Self {
        database.clone()
    }
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl DbExecutor for Database {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        self.inner.execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        self.inner.fetch_all(sql, params)
    }
}

pub struct DbTransaction {
    inner: Box<dyn TransactionDriver>,
}

impl DbTransaction {
    pub fn new(inner: Box<dyn TransactionDriver>) -> Self {
        Self { inner }
    }

    pub async fn commit(self) -> Result<(), DbError> {
        self.inner.commit().await
    }
}

impl DbExecutor for DbTransaction {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        self.inner.execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        self.inner.fetch_all(sql, params)
    }
}

pub mod row_mapping {
    use super::*;

    pub fn cells(values: impl IntoIterator<Item = (impl Into<String>, DbCell)>) -> DbRow {
        DbRow::new(
            values
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        )
    }
}

#[cfg(feature = "driver-sqlx")]
pub mod sqlx;

#[cfg(feature = "driver-tokio-postgres")]
pub mod tokio_postgres;
