use super::{GraphileWorkerMigration, MigrationStatements};

pub const M000006_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000006",
    is_breaking: false,
    stmts: MigrationStatements::Delimited(include_str!("m000006.sql")),
};
