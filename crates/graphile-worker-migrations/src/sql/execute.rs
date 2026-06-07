use std::future::Future;

use graphile_worker_database::{DbError, DbExecutorArg, DbParams, Schema};
use graphile_worker_migrations_core::GraphileWorkerMigration;

pub trait MigrationExecuteExt {
    fn execute<'a, E, S>(
        &'a self,
        executor: E,
        schema: S,
    ) -> impl Future<Output = Result<(), DbError>> + 'a
    where
        E: DbExecutorArg + 'a,
        S: Into<Schema> + 'a;
}

impl MigrationExecuteExt for GraphileWorkerMigration {
    async fn execute<'a, E, S>(&'a self, mut executor: E, schema: S) -> Result<(), DbError>
    where
        E: DbExecutorArg + 'a,
        S: Into<Schema> + 'a,
    {
        let schema = schema.into();
        for stmt in self.stmts {
            let stmt = stmt.replace(":GRAPHILE_WORKER_SCHEMA", schema.escaped());
            executor.execute(&stmt, DbParams::new()).await?;
        }

        Ok(())
    }
}
