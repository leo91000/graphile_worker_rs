use super::super::*;

#[derive(Debug)]
pub(crate) struct MockDriver {
    pub(crate) rows: Vec<DbRow>,
}

impl DbExecutor for MockDriver {
    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async { Ok(7) })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { Ok(self.rows.clone()) })
    }
}

impl DatabaseDriver for MockDriver {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn begin<'a>(
        &'a self,
    ) -> graphile_worker_database::BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async { Ok(DbTransaction::new(Box::new(MockTransaction))) })
    }

    fn listen<'a>(
        &'a self,
        _channel: &'a str,
    ) -> graphile_worker_database::BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async { Ok(None) })
    }
}

struct MockTransaction;

impl DbExecutor for MockTransaction {
    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async { Ok(3) })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { Ok(vec![row_mapping::cells([("value", DbCell::I32(99))])]) })
    }
}

impl TransactionDriver for MockTransaction {
    fn commit(
        self: Box<Self>,
    ) -> graphile_worker_database::BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async { Ok(()) })
    }
}
