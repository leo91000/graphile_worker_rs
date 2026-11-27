use deadpool_postgres::Client;
use tokio::sync::Mutex;

use super::params::{boxed_params, param_refs};
use super::rows::tokio_row_to_db_row;
use crate::{DbError, DbExecutor, DbParams, DbRow, TransactionDriver};

/// Detaches a connection from the pool unless its owner confirms it is safe
/// to reuse or transfers it to a transaction.
pub(super) struct PendingClient(pub(super) Option<Client>);

impl PendingClient {
    pub(super) fn get(&self) -> &Client {
        self.0.as_ref().expect("pending client must be present")
    }

    fn take(mut self) -> Client {
        self.0.take().expect("pending client must be present")
    }
}

impl Drop for PendingClient {
    fn drop(&mut self) {
        if let Some(client) = self.0.take() {
            drop(Client::take(client));
        }
    }
}

pub(super) struct TokioPostgresTransaction {
    client: Mutex<Option<Client>>,
}

impl TokioPostgresTransaction {
    pub(super) async fn begin(client: Client) -> Result<Self, DbError> {
        // BEGIN can reach the server even if this future is dropped before
        // the response arrives. Establish ownership before sending it.
        let pending = PendingClient(Some(client));
        pending.get().batch_execute("BEGIN").await?;
        Ok(Self {
            client: Mutex::new(Some(pending.take())),
        })
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
            let params = boxed_params(params)?;
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
            let params = boxed_params(params)?;
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
        // Hold the lease until rollback succeeds. Discard the connection if
        // the cleanup task is cancelled or rollback cannot be confirmed.
        let pending = PendingClient(Some(client));
        drop(tokio::spawn(async move {
            if pending.get().batch_execute("ROLLBACK").await.is_ok() {
                drop(pending.take());
            }
        }));
    }
}
