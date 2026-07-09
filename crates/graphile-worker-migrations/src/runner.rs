use graphile_worker_database::{Database, DbError, DbExecutor, DbValue, Schema};
use tracing::{info, warn};

use crate::clash::is_clash_error;
use crate::error::MigrateError;
use crate::pg_version::check_postgres_version;
use crate::sql::{MigrationExecuteExt, GRAPHILE_WORKER_MIGRATIONS};
use crate::state::get_last_migration;

/// Runs the migrations against the database.
pub async fn migrate(
    database: impl Into<Database>,
    schema: impl Into<Schema>,
) -> Result<(), MigrateError> {
    let database = database.into();
    let schema = schema.into();
    let last_migration = get_last_migration(&database, &schema).await?;

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
            let migrations = schema.identifier("migrations");
            let sql = format!("insert into {migrations} (id, breaking) values ($1, $2)");
            let tx = database.begin().await?;
            let migration_insert_result = tx
                .execute(
                    &sql,
                    vec![
                        DbValue::I32(migration_number as i32),
                        DbValue::Bool(migration.is_breaking()),
                    ]
                    .into(),
                )
                .await;

            if let Err(error) = migration_insert_result {
                if is_clash_error(&error) {
                    info!(
                        migration_number,
                        migration_name = migration.name(),
                        "Some other migration runner performed migration {}; continuing.",
                        migration.name()
                    );
                    continue;
                }

                return Err(MigrateError::SqlError(error));
            }

            let result = migration.execute(&tx, &schema).await;
            check_migration_error(migration_number, result)?;

            tx.commit().await?;
        }
    }

    if migrated {
        info!("Migrations complete");
    }

    check_compatible_revision(
        latest_migration,
        latest_breaking_migration,
        highest_migration,
    )
}

fn check_compatible_revision(
    latest_migration: Option<i32>,
    latest_breaking_migration: Option<i32>,
    highest_migration: u32,
) -> Result<(), MigrateError> {
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
