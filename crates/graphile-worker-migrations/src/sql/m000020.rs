use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000020_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000020",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000020.sql")),
};
