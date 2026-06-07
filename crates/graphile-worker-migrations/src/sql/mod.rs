use std::future::Future;

use graphile_worker_database::{DbError, DbExecutor, DbParams, DbTransaction, Schema};
pub use graphile_worker_migrations_core::GraphileWorkerMigration;
use graphile_worker_migrations_macros::include_migrations;

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

pub const GRAPHILE_WORKER_MIGRATIONS: &[GraphileWorkerMigration] = include_migrations!("src/sql");

#[cfg(test)]
mod tests {
    use super::GRAPHILE_WORKER_MIGRATIONS;

    #[test]
    fn generated_migrations_are_ordered() {
        assert_eq!(GRAPHILE_WORKER_MIGRATIONS.len(), 20);
        for (index, migration) in GRAPHILE_WORKER_MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.migration_number(), index as u32 + 1);
            assert!(!migration.stmts.is_empty());
        }
    }

    #[test]
    fn generated_migrations_load_breaking_markers() {
        assert_eq!(
            GRAPHILE_WORKER_MIGRATIONS
                .iter()
                .filter(|migration| migration.is_breaking())
                .map(|migration| migration.migration_number())
                .collect::<Vec<_>>(),
            vec![1, 3, 11, 13, 14, 16]
        );
    }

    #[test]
    fn generated_migration_statements_do_not_include_metadata() {
        assert!(GRAPHILE_WORKER_MIGRATIONS.iter().all(|migration| migration
            .stmts
            .iter()
            .all(|stmt| !stmt.contains("-- graphile-worker-rs:"))));
    }
}
