use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000012_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000012",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000012.sql")),
};
