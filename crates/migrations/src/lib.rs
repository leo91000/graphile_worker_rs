pub mod pg_version;
pub mod sql;

use indoc::formatdoc;
use pg_version::{check_postgres_version, fetch_and_check_postgres_version};
use sql::ARCHIMEDES_MIGRATIONS;
use sqlx::{query, query_as, Acquire, Error as SqlxError, FromRow, PgExecutor, Postgres};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum MigrateError {
    #[error("Error occured while parsing postgres version: {0}")]
    ParseVersionError(#[from] std::num::ParseIntError),
    #[error("This version of Archimedes requires PostgreSQL v12.0 or greater (detected `server_version_num` = {0})")]
    IncompatibleVersion(u32),
    #[error("Database is using Graphile Worker schema revision {} which includes breaking migration {}, but the currently running worker only supports up to revision {}. It would be unsafe to continue; please ensure all versions of Graphile Worker are compatible.", .latest_migration, .latest_breaking_migration, .highest_migration)]
    IncompatbleRevision {
        latest_migration: i32,
        latest_breaking_migration: i32,
        highest_migration: u32,
    },
    #[error("Error occured while migrate: {0}")]
    SqlError(#[from] sqlx::Error),
}

/// Installs the Archimedes schema into the database.
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

#[derive(FromRow, Default)]
pub struct LastMigration {
    server_version_num: String,
    id: Option<i32>,
    biggest_breaking_id: Option<i32>,
}

/// Returns the last migration that was run against the database.
/// It also installs the Archimedes schema if it doesn't exist.
async fn get_last_migration<'e, E>(
    executor: &E,
    escaped_schema: &str,
) -> Result<LastMigration, MigrateError>
where
    E: PgExecutor<'e> + Acquire<'e, Database = Postgres> + Send + Sync + Clone,
{
    let migrations_status_query = formatdoc!(
        r#"
            select current_setting('server_version_num') as server_version_num,
            (select id from {escaped_schema}.migrations order by id desc limit 1) as id,
            (select id from {escaped_schema}.migrations where breaking is true order by id desc limit 1) as biggest_breaking_id;
        "#
    );
    let last_migration_query_result = query_as::<_, LastMigration>(&migrations_status_query)
        .fetch_one(executor.clone())
        .await;
    let last_migration = match last_migration_query_result {
        Err(SqlxError::Database(e)) => {
            let Some(code) = e.code() else {
                return Err(MigrateError::SqlError(SqlxError::Database(e)));
            };

            if code == "42P01" {
                install_schema(executor.clone(), escaped_schema).await?;
                Default::default()
            }

            return Err(MigrateError::SqlError(SqlxError::Database(e)));
        }
        Err(e) => {
            return Err(MigrateError::SqlError(e));
        }
        Ok(row) => row,
    };
    Ok(last_migration)
}

impl LastMigration {
    fn is_before_number(&self, migration_number: u32) -> bool {
        let migration_id = self.id.and_then(|id| id.try_into().ok());
        self.id.is_none() || migration_number > migration_id.unwrap_or(0)
    }
}

/// Runs the migrations against the database.
pub async fn migrate<'e, E>(executor: E, escaped_schema: &str) -> Result<(), MigrateError>
where
    E: PgExecutor<'e> + Acquire<'e, Database = Postgres> + Send + Sync + Clone,
{
    let last_migration = get_last_migration(&executor, escaped_schema).await?;

    check_postgres_version(&last_migration.server_version_num)?;
    let latest_migration = last_migration.id;
    let latest_breaking_migration = last_migration.biggest_breaking_id;

    let mut highest_migration = 0;
    let mut migrated = false;
    for migration in ARCHIMEDES_MIGRATIONS.iter() {
        let migration_number = migration.migration_number();

        if migration_number > highest_migration {
            highest_migration = migration_number;
        }

        if last_migration.is_before_number(migration_number) {
            migrated = true;
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
                .bind(migration_number as i64)
                .bind(migration.is_breaking())
                .execute(tx.as_mut())
                .await?;

            tx.commit().await?;
        }
    }

    if migrated {
        info!("Migrations complete");
    }

    if let Some(latest_breaking_migration) = latest_breaking_migration {
        if highest_migration < latest_breaking_migration as u32 {
            return Err(MigrateError::IncompatbleRevision {
                latest_migration: latest_migration.unwrap_or(0),
                latest_breaking_migration,
                highest_migration,
            });
        }
    }

    if let Some(latest_migration) = latest_migration {
        if highest_migration < latest_migration as u32 {
            warn!(
                latest_migration,
                highest_migration,
                "Database is using Graphile Worker schema revision {}, but the currently running worker only supports up to revision {} which may or may not be compatible. Please ensure all versions of Graphile Worker you're running are compatible, or use Worker Pro which will perform this check for you. Attempting to continue regardless.",
                latest_migration,
                highest_migration,
            );
        }
    }

    Ok(())
}
