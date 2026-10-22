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
    #[cfg(any(feature = "runtime-tokio", feature = "runtime-async-std"))]
    retry_policy: Option<crate::SqlRetryPolicy>,
}

impl Database {
    pub fn new(driver: impl DatabaseDriver + 'static) -> Self {
        Self {
            inner: Arc::new(driver),
            #[cfg(any(feature = "runtime-tokio", feature = "runtime-async-std"))]
            retry_policy: None,
        }
    }

    /// Enables retries on this handle's pooled execute/fetch operations.
    ///
    /// The driver must execute each operation in its own autocommit transaction.
    /// Do not enable this on a custom driver retaining a caller-managed transaction.
    /// Transactions returned by `begin`, raw driver/pool access and LISTEN retain
    /// their original behavior. Clones retain the policy; existing handles do not.
    /// Retries are disabled by default to preserve immediate error propagation.
    #[cfg(any(feature = "runtime-tokio", feature = "runtime-async-std"))]
    pub fn with_retry_policy(mut self, policy: crate::SqlRetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
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
        #[cfg(any(feature = "runtime-tokio", feature = "runtime-async-std"))]
        if let Some(policy) = self.retry_policy {
            return Box::pin(async move {
                policy.run(|| self.inner.execute(sql, params.clone())).await
            });
        }
        self.inner.execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        #[cfg(any(feature = "runtime-tokio", feature = "runtime-async-std"))]
        if let Some(policy) = self.retry_policy {
            return Box::pin(async move {
                policy
                    .run(|| self.inner.fetch_all(sql, params.clone()))
                    .await
            });
        }
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
