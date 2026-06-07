use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000008_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000008",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000008.sql")),
};
