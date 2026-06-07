use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000016_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000016",
    is_breaking: true,
    stmts: MigrationStatements::Delimited(include_str!("m000016.sql")),
};
