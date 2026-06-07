use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000019_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000019",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000019.sql")),
};
