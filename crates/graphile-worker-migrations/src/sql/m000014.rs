use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000014_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000014",
    is_breaking: true,
    stmts: MigrationStatements::Delimited(include_str!("m000014.sql")),
};
