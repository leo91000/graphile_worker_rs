use sqlx::{Postgres, Transaction};

pub mod m000001;
pub mod m000002;
pub mod m000003;
pub mod m000004;
pub mod m000005;
pub mod m000006;
pub mod m000007;
pub mod m000008;
pub mod m000009;
pub mod m000010;
pub mod m000011;
pub mod m000012;
pub mod m000013;
pub mod m000014;
pub mod m000015;
pub mod m000016;
pub mod m000017;
pub mod m000018;

#[derive(Default)]
pub struct GraphileWorkerMigration {
    pub name: &'static str,
    pub is_breaking: bool,
    pub stmts: &'static [&'static str],
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
        tx: &mut Transaction<'_, Postgres>,
        escaped_schema: &str,
    ) -> Result<(), sqlx::Error> {
        for stmt in self.stmts {
            let stmt = stmt.replace(":GRAPHILE_WORKER_SCHEMA", escaped_schema);
            sqlx::query(&stmt).execute(tx.as_mut()).await?;
        }

        Ok(())
    }
}

pub const GRAPHILE_WORKER_MIGRATIONS: &[GraphileWorkerMigration] = &[
    m000001::M000001_MIGRATION,
    m000002::M000002_MIGRATION,
    m000003::M000003_MIGRATION,
    m000004::M000004_MIGRATION,
    m000005::M000005_MIGRATION,
    m000006::M000006_MIGRATION,
    m000007::M000007_MIGRATION,
    m000008::M000008_MIGRATION,
    m000009::M000009_MIGRATION,
    m000010::M000010_MIGRATION,
    m000011::M000011_MIGRATION,
    m000012::M000012_MIGRATION,
    m000013::M000013_MIGRATION,
    m000014::M000014_MIGRATION,
    m000015::M000015_MIGRATION,
    m000016::M000016_MIGRATION,
    m000017::M000017_MIGRATION,
    m000018::M000018_MIGRATION,
];
