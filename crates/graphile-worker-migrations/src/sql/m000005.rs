use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000005_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000005",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000005.sql")),
};
