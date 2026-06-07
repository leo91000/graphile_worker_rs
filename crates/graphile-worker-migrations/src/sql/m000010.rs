use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000010_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000010",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000010.sql")),
};
