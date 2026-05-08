pub mod pg_version;
pub mod sql;

use graphile_worker_database::{Database, DbError, DbExecutor, DbParams, DbValue};
use indoc::formatdoc;
use pg_version::{check_postgres_version, fetch_and_check_postgres_version};
use sql::GRAPHILE_WORKER_MIGRATIONS;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum MigrateError {
    #[error("Error occured while parsing postgres version: {0}")]
    ParseVersionError(#[from] std::num::ParseIntError),
    #[error("This version of Graphile Worker requires PostgreSQL v12.0 or greater (detected `server_version_num` = {0})")]
    IncompatibleVersion(u32),
    #[error("Database is using Graphile Worker schema revision {} which includes breaking migration {}, but the currently running worker only supports up to revision {}. It would be unsafe to continue; please ensure all versions of Graphile Worker are compatible.", .latest_migration, .latest_breaking_migration, .highest_migration)]
    IncompatbleRevision {
        latest_migration: i32,
        latest_breaking_migration: i32,
        highest_migration: u32,
    },
    #[error("Error occured while migrate: {0}")]
    SqlError(#[from] DbError),
    #[error("There are locked jobs present; migration 11 cannot complete. Please ensure all workers are shut down cleanly and all locked jobs and queues are unlocked before attempting this migration.")]
    LockedJobInMigration11,
}

/// Installs the Graphile Worker schema into the database.
async fn install_schema(database: &Database, escaped_schema: &str) -> Result<(), MigrateError> {
    let version = fetch_and_check_postgres_version(database).await?;
    info!(pg_version = version, "Installing Graphile Worker schema");

    let create_schema_query = formatdoc!(
        r#"
            create schema if not exists {escaped_schema};
        "#
    );

    let create_migration_table_query = formatdoc!(
        r#"
            create table if not exists {escaped_schema}.migrations (
                id int primary key, 
                ts timestamptz default now() not null,
                breaking boolean not null default false
            );
        "#
    );

    let tx = database.begin().await?;
    tx.execute(&create_schema_query, DbParams::new()).await?;
    tx.execute(&create_migration_table_query, DbParams::new())
        .await?;
    tx.commit().await?;

    Ok(())
}

#[derive(Debug)]
pub struct LastMigration {
    server_version_num: String,
    id: Option<i32>,
    biggest_breaking_id: Option<i32>,
}

impl Default for LastMigration {
    fn default() -> Self {
        Self {
            server_version_num: String::from("120000"),
            id: None,
            biggest_breaking_id: None,
        }
    }
}

/// Returns the last migration that was run against the database.
/// It also installs the Graphile Worker schema if it doesn't exist.
async fn get_last_migration(
    executor: &Database,
    escaped_schema: &str,
) -> Result<LastMigration, MigrateError> {
    let migrations_status_query = formatdoc!(
        r#"
            select current_setting('server_version_num') as server_version_num,
            (select id from {escaped_schema}.migrations order by id desc limit 1) as id,
            (select id from {escaped_schema}.migrations where breaking is true order by id desc limit 1) as biggest_breaking_id;
        "#
    );
    let last_migration_query_result = executor
        .fetch_one(&migrations_status_query, DbParams::new())
        .await
        .and_then(|row| {
            Ok(LastMigration {
                server_version_num: row.try_get("server_version_num")?,
                id: row.try_get("id")?,
                biggest_breaking_id: row.try_get("biggest_breaking_id")?,
            })
        });
    let last_migration = match last_migration_query_result {
        Err(e) => {
            if e.code() == Some("42P01") {
                info!(error = ?e, schema = escaped_schema, "Graphile Worker schema not found, installing...");
                install_schema(executor, escaped_schema).await?;
                return Ok(Default::default());
            }

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
pub async fn migrate(
    database: impl Into<Database>,
    escaped_schema: &str,
) -> Result<(), MigrateError> {
    let database = database.into();
    let last_migration = get_last_migration(&database, escaped_schema).await?;

    check_postgres_version(&last_migration.server_version_num)?;
    let latest_migration = last_migration.id;
    let latest_breaking_migration = last_migration.biggest_breaking_id;

    let mut highest_migration = 0;
    let mut migrated = false;
    for migration in GRAPHILE_WORKER_MIGRATIONS.iter() {
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
            let mut tx = database.begin().await?;
            let result = migration.execute(&mut tx, escaped_schema).await;
            check_migration_error(migration_number, result)?;
            let sql =
                format!("insert into {escaped_schema}.migrations (id, breaking) values ($1, $2)");
            tx.execute(
                &sql,
                vec![
                    DbValue::I32(migration_number as i32),
                    DbValue::Bool(migration.is_breaking()),
                ]
                .into(),
            )
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

/// If migration number is 11 and code is 22012, it means there are locked jobs present
fn check_migration_error(
    migration_number: u32,
    result: Result<(), DbError>,
) -> Result<(), MigrateError> {
    match (migration_number, result) {
        (11, Err(e)) => {
            if e.code() == Some("22012") {
                return Err(MigrateError::LockedJobInMigration11);
            }

            Err(MigrateError::SqlError(e))
        }
        (_, Err(e)) => Err(MigrateError::SqlError(e)),
        (_, Ok(())) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database_error(code: Option<&'static str>) -> DbError {
        if let Some(code) = code {
            DbError::with_code("test database error", code)
        } else {
            DbError::new("test database error")
        }
    }

    #[test]
    fn last_migration_detects_pending_migrations() {
        assert!(LastMigration::default().is_before_number(1));

        let last_migration = LastMigration {
            id: Some(12),
            ..Default::default()
        };
        assert!(!last_migration.is_before_number(12));
        assert!(last_migration.is_before_number(13));

        let negative_migration = LastMigration {
            id: Some(-1),
            ..Default::default()
        };
        assert!(negative_migration.is_before_number(1));
    }

    #[test]
    fn check_migration_error_handles_generic_results() {
        assert!(check_migration_error(1, Ok(())).is_ok());

        let error = check_migration_error(1, Err(DbError::new("row not found"))).unwrap_err();
        assert!(matches!(error, MigrateError::SqlError(_)));
    }

    #[test]
    fn check_migration_error_detects_locked_jobs_in_migration_11() {
        let error = check_migration_error(11, Err(database_error(Some("22012")))).unwrap_err();
        assert!(matches!(error, MigrateError::LockedJobInMigration11));
    }

    #[test]
    fn check_migration_error_keeps_migration_11_database_errors_without_code() {
        let error = check_migration_error(11, Err(database_error(None))).unwrap_err();
        assert!(matches!(error, MigrateError::SqlError(_)));
    }

    #[test]
    fn check_migration_error_keeps_other_migration_11_database_errors() {
        let error = check_migration_error(11, Err(database_error(Some("12345")))).unwrap_err();
        assert!(matches!(error, MigrateError::SqlError(_)));
    }
}
