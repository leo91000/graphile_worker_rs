use futures::StreamExt;
use sqlx::PgPool;

use super::transaction::SqlxTransaction;
use crate::{Database, DatabaseDriver, DbError, DbTransaction, Notification, NotificationStream};

#[derive(Clone, Debug)]
pub struct SqlxDatabase {
    pub(super) pool: PgPool,
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

impl DatabaseDriver for SqlxDatabase {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn begin<'a>(&'a self) -> crate::BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async move {
            let tx = self.pool.begin().await?;
            Ok(DbTransaction::new(Box::new(SqlxTransaction::new(tx))))
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
