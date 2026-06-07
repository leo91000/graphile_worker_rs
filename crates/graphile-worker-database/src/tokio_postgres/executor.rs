use ::tokio_postgres::{Client, GenericClient, Transaction};
use deadpool_postgres::Pool;

use super::params::{boxed_params, param_refs};
use super::rows::tokio_row_to_db_row;
use super::TokioPostgresDatabase;
use crate::{DbError, DbExecutor, DbExecutorArg, DbParams, DbRow};

impl From<::tokio_postgres::Error> for DbError {
    fn from(error: ::tokio_postgres::Error) -> Self {
        if let Some(db_error) = error.as_db_error() {
            return DbError::with_code(error.to_string(), db_error.code().code());
        }

        DbError::new(error.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for DbError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        DbError::new(error.to_string())
    }
}

async fn execute_with_client(
    client: &(impl GenericClient + Sync),
    sql: &str,
    params: DbParams,
) -> Result<u64, DbError> {
    let params = boxed_params(params);
    let refs = param_refs(&params);
    client.execute(sql, &refs).await.map_err(Into::into)
}

async fn fetch_all_with_client(
    client: &(impl GenericClient + Sync),
    sql: &str,
    params: DbParams,
) -> Result<Vec<DbRow>, DbError> {
    let params = boxed_params(params);
    let refs = param_refs(&params);
    let rows = client.query(sql, &refs).await?;
    rows.into_iter().map(tokio_row_to_db_row).collect()
}

macro_rules! impl_executor_arg_for_generic_client {
    ($target:ty, $client:expr) => {
        impl DbExecutorArg for $target {
            fn execute<'a>(
                &'a mut self,
                sql: &'a str,
                params: DbParams,
            ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
                Box::pin(async move { execute_with_client($client, sql, params).await })
            }

            fn fetch_all<'a>(
                &'a mut self,
                sql: &'a str,
                params: DbParams,
            ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
                Box::pin(async move { fetch_all_with_client($client, sql, params).await })
            }
        }
    };
}

impl DbExecutor for TokioPostgresDatabase {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let client = self.pool.get().await?;
            execute_with_client(&*client, sql, params).await
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let client = self.pool.get().await?;
            fetch_all_with_client(&*client, sql, params).await
        })
    }
}

impl_executor_arg_for_generic_client!(&Client, *self);
impl_executor_arg_for_generic_client!(&Transaction<'_>, *self);
impl_executor_arg_for_generic_client!(&mut Transaction<'_>, &**self);
impl_executor_arg_for_generic_client!(&deadpool_postgres::Client, &**self);

impl DbExecutorArg for &Pool {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let client = self.get().await?;
            execute_with_client(&*client, sql, params).await
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let client = self.get().await?;
            fetch_all_with_client(&*client, sql, params).await
        })
    }
}
