use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000017_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000017",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000017.sql")),
};
