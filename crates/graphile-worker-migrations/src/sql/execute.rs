use std::future::Future;

use graphile_worker_database::{DbError, DbExecutor, DbParams, DbTransaction, Schema};
use graphile_worker_migrations_core::GraphileWorkerMigration;

pub trait MigrationExecutor {
    fn execute_statement<'a>(
        &'a mut self,
        stmt: &'a str,
    ) -> impl Future<Output = Result<(), DbError>> + 'a;
}

impl MigrationExecutor for DbTransaction {
    async fn execute_statement(&mut self, stmt: &str) -> Result<(), DbError> {
        self.execute(stmt, DbParams::new()).await?;
        Ok(())
    }
}

#[cfg(feature = "driver-sqlx")]
impl MigrationExecutor for sqlx::Transaction<'_, sqlx::Postgres> {
    async fn execute_statement(&mut self, stmt: &str) -> Result<(), DbError> {
        sqlx::query(sqlx::AssertSqlSafe(stmt))
            .execute(self.as_mut())
            .await?;
        Ok(())
    }
}

pub trait MigrationExecuteExt {
    fn execute<'a, E, S>(
        &'a self,
        tx: &'a mut E,
        schema: S,
    ) -> impl Future<Output = Result<(), DbError>> + 'a
    where
        E: MigrationExecutor + 'a,
        S: Into<Schema> + 'a;
}

impl MigrationExecuteExt for GraphileWorkerMigration {
    async fn execute<'a, E, S>(&'a self, tx: &'a mut E, schema: S) -> Result<(), DbError>
    where
        E: MigrationExecutor + 'a,
        S: Into<Schema> + 'a,
    {
        let schema = schema.into();
        for stmt in self.stmts {
            let stmt = stmt.replace(":GRAPHILE_WORKER_SCHEMA", schema.escaped());
            tx.execute_statement(&stmt).await?;
        }

        Ok(())
    }
}
