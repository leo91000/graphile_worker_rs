use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000007_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000007",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000007.sql")),
};
