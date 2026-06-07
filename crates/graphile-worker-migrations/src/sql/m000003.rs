use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000003_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000003",
    is_breaking: true,
    stmts: MigrationStatements::Delimited(include_str!("m000003.sql")),
};
