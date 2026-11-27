use futures::lock::Mutex;
use sqlx::{pool::PoolConnection, Postgres};

use super::{params::bind_params, rows::sqlx_row_to_db_row};
use crate::{BoxFuture, DbError, DbExecutor, DbParams, DbRow};

pub(super) struct SqlxSession {
    connection: Mutex<PoolConnection<Postgres>>,
    prepared_statements: bool,
}

impl SqlxSession {
    pub(super) fn new(mut connection: PoolConnection<Postgres>, prepared_statements: bool) -> Self {
        connection.close_on_drop();
        Self {
            connection: Mutex::new(connection),
            prepared_statements,
        }
    }
}

impl DbExecutor for SqlxSession {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let mut connection = self.connection.lock().await;
            let result = bind_params(sql, &params)?
                .persistent(self.prepared_statements)
                .execute(&mut **connection)
                .await?;
            Ok(result.rows_affected())
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let mut connection = self.connection.lock().await;
            let rows = bind_params(sql, &params)?
                .persistent(self.prepared_statements)
                .fetch_all(&mut **connection)
                .await?;
            rows.into_iter().map(sqlx_row_to_db_row).collect()
        })
    }
}
