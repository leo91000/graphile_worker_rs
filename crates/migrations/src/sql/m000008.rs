use indoc::indoc;

use super::ArchimedesMigration;

pub const M000008_MIGRATION: ArchimedesMigration = ArchimedesMigration {
    name: "m000008",
    is_breaking: false,
    stmts: &[
        indoc! {r#"
            create table :ARCHIMEDES_SCHEMA.known_crontabs (
                identifier text not null primary key,
                known_since timestamptz not null,
                last_execution timestamptz
            );
        "#},
        indoc! {r#"
            alter table :ARCHIMEDES_SCHEMA.known_crontabs enable row level security;
        "#},
    ],
};
