pub const M000008_MIGRATION: &[&str] = &[
    r#"
        create table :ARCHIMEDES_SCHEMA.known_crontabs (
            identifier text not null primary key,
            known_since timestamptz not null,
            last_execution timestamptz
        );
    "#,
    r#"
        alter table :ARCHIMEDES_SCHEMA.known_crontabs enable row level security;
    "#,
];
