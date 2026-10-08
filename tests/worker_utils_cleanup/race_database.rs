//! Keep deliberate test pauses independent of the production lock timeout.
//! The separate timeout regression uses the unwrapped database.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use graphile_worker_database::{
    BoxFuture, Database, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
    NotificationStream, TransactionDriver,
};

#[derive(Debug, Default)]
pub(super) struct RefreshGate {
    pub started: tokio::sync::Notify,
    pub finish: tokio::sync::Notify,
    used: AtomicBool,
}

#[derive(Debug)]
pub(super) struct RaceDatabase {
    database: Database,
    refresh_gate: Option<Arc<RefreshGate>>,
}

impl RaceDatabase {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            refresh_gate: None,
        }
    }

    pub fn with_refresh_gate(mut self, gate: Arc<RefreshGate>) -> Self {
        self.refresh_gate = Some(gate);
        self
    }
}

impl DbExecutor for RaceDatabase {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        self.database.execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = self.database.fetch_all(sql, params).await?;
            if let Some(gate) = &self.refresh_gate {
                if sql.starts_with("select id, identifier from")
                    && !gate.used.swap(true, Ordering::SeqCst)
                {
                    gate.started.notify_one();
                    gate.finish.notified().await;
                }
            }
            Ok(rows)
        })
    }
}

impl DatabaseDriver for RaceDatabase {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn begin(&self) -> BoxFuture<'_, Result<DbTransaction, DbError>> {
        Box::pin(async {
            Ok(DbTransaction::new(Box::new(RaceTransaction(
                self.database.begin().await?,
            ))))
        })
    }

    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(self.database.listen(channel))
    }
}

struct RaceTransaction(DbTransaction);

impl DbExecutor for RaceTransaction {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        // Only the fault-injection fixtures get more time to coordinate their
        // advisory pauses. Production timeout behavior has its own regression.
        let sql = if sql == "SET LOCAL lock_timeout = '1s'" {
            "SET LOCAL lock_timeout = '10s'"
        } else {
            sql
        };
        self.0.execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        self.0.fetch_all(sql, params)
    }
}

impl TransactionDriver for RaceTransaction {
    fn commit(self: Box<Self>) -> BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async move { self.0.commit().await })
    }
}
