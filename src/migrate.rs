use crate::errors::Result;
use async_trait::async_trait;
use sqlx::{Row, query, query_file, Executor, Postgres, Error as SqlxError};

async fn install_schema<'e, E: Executor<'e, Database = Postgres> + Send + Sync + Clone>(
    executor: &E,
    escaped_schema: &str,
) -> Result<()> {
    let create_schema_query = format!(
        r#"
            create schema {escaped_schema};
            create table {escaped_schema}.migrations (
                id int primary key, 
                ts timestamptz default now() not null
            );
        "#
    );

    query(&create_schema_query)
        .execute((*executor).clone())
        .await?;

    Ok(())
}
async fn escape_identifier<'e, E: Executor<'e, Database = Postgres> + Send + Sync + Clone>(
    executor: &E,
    identifier: &str,
) -> Result<String> {
    let escaped_identifier = query!(
        "select format('%I', $1::text) as escaped_identifier",
        identifier
    )
    .fetch_one((*executor).clone())
    .await?
    .escaped_identifier
    .unwrap();

    Ok(escaped_identifier)
}

async fn migrate<'e, E: Executor<'e, Database = Postgres> + Send + Sync + Clone>(
    executor: &E,
    schema: &str,
) -> Result<()> {
    let escaped_schema = escape_identifier(executor, schema).await?;

    let migrations_status_query = format!("select id from {escaped_schema}.migrations order by id desc limit 1");
    let last_migration_query_result = query(&migrations_status_query).fetch_optional((*executor).clone()).await;

    let mut last_migration: Option<String> = None;
    match last_migration_query_result {
        Err(SqlxError::Database(e)) => {
            if let Some(code) = e.code() {
                if code.eq("42P01") {
                    install_schema(executor, escaped_schema.as_str()).await?;
                } else {
                    return Err(SqlxError::Database(e).into());
                }
            }
        },
        Err(e) => {
            return Err(e.into());
        },
        Ok(Some(row)) => {
            last_migration = Some(row.get("id"));
        },
        _ => {}
    }

    Ok(())
}

#[async_trait]
pub trait ArchimedesMigration {
    async fn migrate(&self, schema: &str) -> Result<()>;
}

#[async_trait]
impl<'e, E> ArchimedesMigration for E
where
    E: Executor<'e, Database = Postgres> + Send + Sync + Clone,
{
    async fn migrate(&self, schema: &str) -> Result<()> {
        migrate(self, schema).await
    }
}
