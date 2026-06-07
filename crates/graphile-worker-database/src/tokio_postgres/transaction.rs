use tokio::sync::Mutex;

use super::params::{boxed_params, param_refs};
use super::rows::tokio_row_to_db_row;
use crate::{DbError, DbExecutor, DbParams, DbRow, TransactionDriver};

pub(super) struct TokioPostgresTransaction {
    client: Mutex<Option<deadpool_postgres::Client>>,
}

impl TokioPostgresTransaction {
    pub(super) fn new(client: deadpool_postgres::Client) -> Self {
        Self {
            client: Mutex::new(Some(client)),
        }
    }
}

impl DbExecutor for TokioPostgresTransaction {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let mut guard = self.client.lock().await;
            let client = guard
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            client.execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let mut guard = self.client.lock().await;
            let client = guard
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = client.query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
    }
}

impl TransactionDriver for TokioPostgresTransaction {
    fn commit(self: Box<Self>) -> crate::BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async move {
            let mut guard = self.client.lock().await;
            let client = guard
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            client.batch_execute("COMMIT").await?;
            guard.take();
            Ok(())
        })
    }
}

impl Drop for TokioPostgresTransaction {
    fn drop(&mut self) {
        let Some(client) = self.client.get_mut().take() else {
            return;
        };
        drop(tokio::spawn(async move {
            let _ = client.batch_execute("ROLLBACK").await;
        }));
    }
}
