use typed_builder::TypedBuilder;
use sqlx::postgres::PgPool;
use getset::Getters;

#[derive(Getters, TypedBuilder)]
#[getset(get = "pub")]
pub struct ArchimedesOptions {
    pg_schema: Option<String>,
}

#[derive(Getters, TypedBuilder)]
#[getset(get = "pub")]
pub struct ArchimedesContext {
    pool: PgPool,
    options: ArchimedesOptions,
}

