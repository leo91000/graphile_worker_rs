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
    /// use graphile_worker_migrations_core::GraphileWorkerMigration;
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
}
