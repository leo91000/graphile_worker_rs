use super::ArchimedesMigration;

pub const M000010_MIGRATION: ArchimedesMigration = ArchimedesMigration {
    name: "m000010",
    is_breaking: false,
    stmts: &["alter table :ARCHIMEDES_SCHEMA.jobs alter column queue_name drop default;"],
};
