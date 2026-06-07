use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::{BoxFuture, DbError, DbExecutor, DbParams, DbRow, NotificationStream};

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
