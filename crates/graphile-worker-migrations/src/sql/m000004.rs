use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000004_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000004",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000004.sql")),
};
