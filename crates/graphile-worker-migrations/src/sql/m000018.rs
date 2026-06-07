use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000018_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000018",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000018.sql")),
};
