use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000013_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000013",
    is_breaking: true,
    stmts: MigrationStatements::Delimited(include_str!("m000013.sql")),
};
