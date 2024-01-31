use indoc::indoc;

use super::GraphileWorkerMigration;

pub const M000008_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000008",
    is_breaking: false,
    stmts: &[
        indoc! {r#"
            create table :GRAPHILE_WORKER_SCHEMA.known_crontabs (
                identifier text not null primary key,
                known_since timestamptz not null,
                last_execution timestamptz
            );
        "#},
        indoc! {r#"
            alter table :GRAPHILE_WORKER_SCHEMA.known_crontabs enable row level security;
        "#},
    ],
};
