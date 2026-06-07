use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000002_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000002",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000002.sql")),
};
