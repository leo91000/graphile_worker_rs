use graphile_worker_database::{DbError, DbExecutor, DbParams, DbTransaction, Schema};

#[allow(async_fn_in_trait)]
pub trait MigrationExecutor {
    async fn execute_statement(&mut self, stmt: &str) -> Result<(), DbError>;
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

macro_rules! define_migrations {
    ($(($module:ident, $constant:ident)),+ $(,)?) => {
        $(pub mod $module;)+

        pub const GRAPHILE_WORKER_MIGRATIONS: &[GraphileWorkerMigration] = &[
            $($module::$constant),+
        ];
    };
}

define_migrations! {
    (m000001, M000001_MIGRATION),
    (m000002, M000002_MIGRATION),
    (m000003, M000003_MIGRATION),
    (m000004, M000004_MIGRATION),
    (m000005, M000005_MIGRATION),
    (m000006, M000006_MIGRATION),
    (m000007, M000007_MIGRATION),
    (m000008, M000008_MIGRATION),
    (m000009, M000009_MIGRATION),
    (m000010, M000010_MIGRATION),
    (m000011, M000011_MIGRATION),
    (m000012, M000012_MIGRATION),
    (m000013, M000013_MIGRATION),
    (m000014, M000014_MIGRATION),
    (m000015, M000015_MIGRATION),
    (m000016, M000016_MIGRATION),
    (m000017, M000017_MIGRATION),
    (m000018, M000018_MIGRATION),
    (m000019, M000019_MIGRATION),
    (m000020, M000020_MIGRATION),
}

#[derive(Clone, Copy)]
pub enum MigrationStatements {
    Static(&'static [&'static str]),
    Delimited(&'static str),
}

impl Default for MigrationStatements {
    fn default() -> Self {
        Self::Static(&[])
    }
}

impl MigrationStatements {
    pub const DELIMITER: &'static str = "-- graphile-worker-rs:statement";

    pub fn iter(self) -> MigrationStatementIter {
        match self {
            Self::Static(stmts) => MigrationStatementIter::Static(stmts.iter()),
            Self::Delimited(script) => MigrationStatementIter::Delimited {
                statements: script.split(Self::DELIMITER),
            },
        }
    }
}

pub enum MigrationStatementIter {
    Static(std::slice::Iter<'static, &'static str>),
    Delimited {
        statements: std::str::Split<'static, &'static str>,
    },
}

impl Iterator for MigrationStatementIter {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Static(statements) => statements.next().copied(),
            Self::Delimited { statements } => statements
                .by_ref()
                .map(str::trim)
                .find(|statement| !statement.is_empty()),
        }
    }
}

#[derive(Default)]
pub struct GraphileWorkerMigration {
    pub name: &'static str,
    pub is_breaking: bool,
    pub stmts: MigrationStatements,
}

impl GraphileWorkerMigration {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn is_breaking(&self) -> bool {
        self.is_breaking
    }

    /// Parses the migration name and returns the migration number.
    /// ```rust
    /// use graphile_worker_migrations::sql::GraphileWorkerMigration;
    /// let migration = GraphileWorkerMigration {
    ///     name: "m000001_initial",
    ///     ..Default::default()
    /// };
    /// assert_eq!(migration.migration_number(), 1);
    /// let migration = GraphileWorkerMigration {
    ///     name: "m000002",
    ///     ..Default::default()
    /// };
    /// assert_eq!(migration.migration_number(), 2);
    /// ```
    pub fn migration_number(&self) -> u32 {
        self.name[1..7].parse().expect("Invalid migration name")
    }

    pub async fn execute(
        &self,
        tx: &mut impl MigrationExecutor,
        schema: impl Into<Schema>,
    ) -> Result<(), DbError> {
        let schema = schema.into();
        for stmt in self.stmts.iter() {
            let stmt = stmt.replace(":GRAPHILE_WORKER_SCHEMA", schema.escaped());
            tx.execute_statement(&stmt).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MigrationStatements;

    #[test]
    fn static_statements_iterate_in_order() {
        let statements = MigrationStatements::Static(&["select 1", "select 2"]);

        assert_eq!(
            statements.iter().collect::<Vec<_>>(),
            vec!["select 1", "select 2"]
        );
    }

    #[test]
    fn delimited_statements_trim_and_skip_empty_sections() {
        let statements = MigrationStatements::Delimited(
            "\n  select 1;  \n-- graphile-worker-rs:statement\n\n-- graphile-worker-rs:statement\n  select 2;\n",
        );

        assert_eq!(
            statements.iter().collect::<Vec<_>>(),
            vec!["select 1;", "select 2;"]
        );
    }
}
