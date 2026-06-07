use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000015_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000015",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000015.sql")),
};
