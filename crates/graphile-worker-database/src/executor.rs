use crate::{BoxFuture, DbError, DbParams, DbRow};

pub trait DbExecutor: Send + Sync {
    fn execute<'a>(&'a self, sql: &'a str, params: DbParams)
        -> BoxFuture<'a, Result<u64, DbError>>;

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>>;

    fn fetch_optional<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Option<DbRow>, DbError>> {
        Box::pin(async move {
            let rows = self.fetch_all(sql, params).await?;
            Ok(rows.into_iter().next())
        })
    }

    fn fetch_one<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<DbRow, DbError>> {
        Box::pin(async move {
            self.fetch_optional(sql, params).await?.ok_or_else(|| {
                DbError::new("query returned no rows when exactly one row was expected")
            })
        })
    }
}

pub trait DbExecutorArg: Send {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>>;

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>>;

    fn fetch_optional<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Option<DbRow>, DbError>>
    where
        Self: Send + 'a,
    {
        Box::pin(async move {
            let rows = self.fetch_all(sql, params).await?;
            Ok(rows.into_iter().next())
        })
    }

    fn fetch_one<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<DbRow, DbError>>
    where
        Self: Send + 'a,
    {
        Box::pin(async move {
            self.fetch_optional(sql, params).await?.ok_or_else(|| {
                DbError::new("query returned no rows when exactly one row was expected")
            })
        })
    }
}

impl<T: DbExecutor + ?Sized> DbExecutorArg for &T {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        DbExecutor::execute(*self, sql, params)
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        DbExecutor::fetch_all(*self, sql, params)
    }
}

impl<T: DbExecutorArg + ?Sized> DbExecutorArg for &mut T {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        (**self).execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        (**self).fetch_all(sql, params)
    }
}
