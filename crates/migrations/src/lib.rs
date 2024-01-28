pub mod pg_version;
mod sql;

use indoc::formatdoc;
use pg_version::fetch_and_check_postgres_version;
use sql::ARCHIMEDES_MIGRATIONS;
use sqlx::{query, Acquire, Error as SqlxError, PgExecutor, Postgres, Row};
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum MigrateError {
    #[error("Error occured while parsing postgres version: {0}")]
    ParseVersionError(#[from] std::num::ParseIntError),
    #[error("This version of Archimedes requires PostgreSQL v12.0 or greater (detected `server_version_num` = {0})")]
    IncompatibleVersion(u32),
    #[error("Error occured while migrate: {0}")]
    SqlError(#[from] sqlx::Error),
}

async fn install_schema<'e, E>(executor: E, escaped_schema: &str) -> Result<(), MigrateError>
where
    E: PgExecutor<'e> + Acquire<'e, Database = Postgres> + Clone,
{
    let version = fetch_and_check_postgres_version(executor.clone()).await?;
    info!(pg_version = version, "Installing Archimedes schema");

    let create_schema_query = formatdoc!(
        r#"
            create schema {escaped_schema};
        "#
    );

    let create_migration_table_query = formatdoc!(
        r#"
            create table {escaped_schema}.migrations (
                id int primary key, 
                ts timestamptz default now() not null,
                breaking boolean not null default false
            );
        "#
    );

    let mut tx = executor.begin().await?;
    query(&create_schema_query).execute(tx.as_mut()).await?;
    query(&create_migration_table_query)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;

    Ok(())
}

pub async fn migrate<'e, E>(executor: E, escaped_schema: &str) -> Result<(), MigrateError>
where
    E: PgExecutor<'e> + Acquire<'e, Database = Postgres> + Send + Sync + Clone,
{
    let migrations_status_query =
        format!("select id from {escaped_schema}.migrations order by id desc limit 1");
    let last_migration_query_result = query(&migrations_status_query)
        .fetch_optional(executor.clone())
        .await;

    let last_migration = match last_migration_query_result {
        Err(SqlxError::Database(e)) => {
            let Some(code) = e.code() else {
                return Err(MigrateError::SqlError(SqlxError::Database(e)));
            };

            if code == "42P01" {
                install_schema(executor.clone(), escaped_schema).await?;
            } else {
                return Err(MigrateError::SqlError(SqlxError::Database(e)));
            }

            None
        }
        Err(e) => {
            return Err(MigrateError::SqlError(e));
        }
        Ok(optional_row) => optional_row.map(|row| row.get("id")),
    };

    for (i, migration) in ARCHIMEDES_MIGRATIONS.iter().enumerate() {
        let migration_number = (i + 1) as i32;

        if last_migration.is_none() || migration_number > last_migration.unwrap() {
            info!(
                migration_number,
                migration_name = migration.name(),
                is_breaking_migration = migration.is_breaking(),
                "Running {} migration {}",
                if migration.is_breaking() {
                    "breaking"
                } else {
                    "backwards-compatible"
                },
                migration.name(),
            );
            let mut tx = executor.clone().begin().await?;
            migration.execute(&mut tx, escaped_schema).await?;
            let sql =
                format!("insert into {escaped_schema}.migrations (id, breaking) values ($1, $2)");
            query(&sql)
                .bind(migration_number)
                .bind(migration.is_breaking())
                .execute(tx.as_mut())
                .await?;

            tx.commit().await?;
        }
    }

    Ok(())
}
