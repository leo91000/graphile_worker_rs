use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000011_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000011",
    is_breaking: true,
    stmts: MigrationStatements::Delimited(include_str!("m000011.sql")),
};
