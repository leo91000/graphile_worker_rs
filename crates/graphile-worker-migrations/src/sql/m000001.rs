use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000001_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000001",
    is_breaking: true,
    stmts: MigrationStatements::Delimited(include_str!("m000001.sql")),
};
