use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::{BoxFuture, DbError, DbExecutor, DbParams, DbRow, NotificationStream};

pub trait DatabaseDriver: DbExecutor + fmt::Debug + Any {
    fn as_any(&self) -> &dyn Any;

    fn begin<'a>(&'a self) -> BoxFuture<'a, Result<DbTransaction, DbError>>;

    /// Acquires one PostgreSQL session for operations that must share a backend.
    ///
    /// The driver must discard the session when the executor is dropped, including
    /// during cancellation, since callers can leave session state or open transactions.
    fn acquire_session(&self) -> BoxFuture<'_, Result<Box<dyn DbExecutor>, DbError>> {
        Box::pin(async {
            Err(DbError::new(
                "this database driver does not support pinned sessions",
            ))
        })
    }

    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>>;

    /// Listen on a primary channel and, where supported, additional channels
    /// on the same connection. Single-channel drivers retain their primary
    /// subscription by default.
    fn listen_with_channels<'a>(
        &'a self,
        primary: &'a str,
        _additional: &'a [&'a str],
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        self.listen(primary)
    }
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
    /// Wraps a pooled driver with immediate error propagation by default.
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

    /// Acquires a pinned session. Pooled statement retries do not apply to it.
    pub async fn acquire_session(&self) -> Result<Box<dyn DbExecutor>, DbError> {
        self.inner.acquire_session().await
    }

    pub async fn listen(&self, channel: &str) -> Result<Option<NotificationStream>, DbError> {
        self.inner.listen(channel).await
    }

    /// Listen on a primary channel and any additional channels supported by the driver.
    pub async fn listen_with_channels(
        &self,
        primary: &str,
        additional: &[&str],
    ) -> Result<Option<NotificationStream>, DbError> {
        self.inner.listen_with_channels(primary, additional).await
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
    /// Executes pooled SQL with the optional policy, preserving bindings on retries.
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

    /// Fetches pooled rows with the optional policy and original error diagnostics.
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
