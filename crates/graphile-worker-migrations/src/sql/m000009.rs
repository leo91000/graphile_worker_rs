use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000009_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000009",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000009.sql")),
};
