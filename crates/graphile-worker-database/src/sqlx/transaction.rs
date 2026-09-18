use futures::lock::Mutex;
use sqlx::Postgres;

use super::params::bind_params;
use super::rows::sqlx_row_to_db_row;
use crate::{DbError, DbExecutor, DbParams, DbRow, TransactionDriver};

pub(super) struct SqlxTransaction {
    prepared_statements: bool,
    tx: Mutex<Option<sqlx::Transaction<'static, Postgres>>>,
}

impl SqlxTransaction {
    /// Carries the database wrapper's statement policy into a SQLx transaction.
    pub(super) fn new(tx: sqlx::Transaction<'static, Postgres>, prepared_statements: bool) -> Self {
        Self {
            tx: Mutex::new(Some(tx)),
            prepared_statements,
        }
    }
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
                .persistent(self.prepared_statements)
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

            let rows = bind_params(sql, &params)
                .persistent(self.prepared_statements)
                .fetch_all(tx.as_mut())
                .await?;
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
