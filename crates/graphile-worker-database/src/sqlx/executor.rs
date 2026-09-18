use sqlx::{Executor, PgPool, Postgres};

use super::driver::SqlxDatabase;
use super::params::bind_params;
use super::rows::sqlx_row_to_db_row;
use crate::{DbError, DbExecutor, DbExecutorArg, DbParams, DbRow};

/// Executes bound SQL with the caller's persistent-statement policy.
async fn execute_with_executor<'e, E>(
    executor: E,
    sql: &str,
    params: DbParams,
    prepared_statements: bool,
) -> Result<u64, DbError>
where
    E: Executor<'e, Database = Postgres>,
{
    bind_params(sql, &params)
        .persistent(prepared_statements)
        .execute(executor)
        .await
        .map(|result| result.rows_affected())
        .map_err(Into::into)
}

/// Fetches typed driver rows with the caller's persistent-statement policy.
async fn fetch_all_with_executor<'e, E>(
    executor: E,
    sql: &str,
    params: DbParams,
    prepared_statements: bool,
) -> Result<Vec<DbRow>, DbError>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = bind_params(sql, &params)
        .persistent(prepared_statements)
        .fetch_all(executor)
        .await?;
    rows.into_iter().map(sqlx_row_to_db_row).collect()
}

impl DbExecutor for SqlxDatabase {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            execute_with_executor(&self.pool, sql, params, self.prepared_statements).await
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            fetch_all_with_executor(&self.pool, sql, params, self.prepared_statements).await
        })
    }
}

impl DbExecutor for PgPool {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move { execute_with_executor(self, sql, params, true).await })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move { fetch_all_with_executor(self, sql, params, true).await })
    }
}

impl DbExecutorArg for &mut sqlx::Transaction<'_, Postgres> {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move { execute_with_executor((**self).as_mut(), sql, params, true).await })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move { fetch_all_with_executor((**self).as_mut(), sql, params, true).await })
    }
}

impl DbExecutorArg for &mut sqlx::PgConnection {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move { execute_with_executor(&mut **self, sql, params, true).await })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move { fetch_all_with_executor(&mut **self, sql, params, true).await })
    }
}
