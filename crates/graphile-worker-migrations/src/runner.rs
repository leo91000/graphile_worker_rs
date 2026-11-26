use std::time::{Duration, Instant};

use graphile_worker_database::{Database, DbError, DbExecutor, DbParams, DbValue, Schema};
use graphile_worker_runtime::timeout_at;
use tracing::{info, warn};

use crate::clash::is_clash_error;
use crate::connection::MigrationConnection;
use crate::error::MigrateError;
use crate::hooks::{MigrationHooks, MigrationPhase, MigrationRun};
use crate::pg_version::check_postgres_version;
use crate::sql::{MigrationExecuteExt, GRAPHILE_WORKER_MIGRATIONS};
use crate::state::{get_last_migration, read_last_migration};
use crate::GraphileWorkerMigration;

/// Notification channel used by Graphile Worker to announce committed migrations.
pub const MIGRATION_CHANNEL: &str = "worker:migrate";

/// Runs the migrations against the database.
pub async fn migrate(
    database: impl Into<Database>,
    schema: impl Into<Schema>,
) -> Result<(), MigrateError> {
    migrate_with_hooks(database, schema, &MigrationHooks::default()).await
}

/// Runs migrations with callbacks sharing one PostgreSQL session.
///
/// An empty registry retains the pooled migration path. A nonempty registry
/// requires a driver supporting pinned sessions.
pub async fn migrate_with_hooks(
    database: impl Into<Database>,
    schema: impl Into<Schema>,
    hooks: &MigrationHooks,
) -> Result<(), MigrateError> {
    let database = database.into();
    let schema = schema.into();
    let connection = if hooks.is_empty() {
        MigrationConnection::Pooled(&database)
    } else {
        MigrationConnection::Pinned(database.acquire_session().await?)
    };
    let mut run = MigrationRun {
        connection,
        hooks,
        postgres_version: 0,
        scratchpad: Default::default(),
    };
    let last_migration = get_last_migration(&mut run, &schema).await?;
    run.postgres_version = check_postgres_version(&last_migration.server_version_num)?;
    run.emit(&schema, MigrationPhase::BeforeMigrate).await?;
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
            run_revision(&mut run, &schema, migration).await?;
        }
    }

    if migrated {
        info!("Migrations complete");
    }

    check_compatible_revision(
        latest_migration,
        latest_breaking_migration,
        highest_migration,
    )?;
    run.emit(&schema, MigrationPhase::AfterMigrate).await?;
    Ok(())
}

async fn run_revision(
    run: &mut MigrationRun<'_>,
    schema: &Schema,
    migration: &GraphileWorkerMigration,
) -> Result<(), MigrateError> {
    let revision = migration.migration_number();
    let migrations = schema.identifier("migrations");
    let insert = format!("insert into {migrations} (id, breaking) values ($1, $2)");
    let tx = run.connection.begin().await?;
    let executor = tx.executor();
    let mut inserted = false;
    let mut executing_migration = false;
    let result = async {
        executor.execute(
            &insert,
            vec![DbValue::I32(revision as i32), DbValue::Bool(migration.is_breaking())].into(),
        ).await?;
        inserted = true;
        executing_migration = true;
        migration.execute(executor, schema).await?;
        executing_migration = false;

        // Notify listeners only when the schema change and its ledger row commit.
        executor.execute(
            "select pg_notify($1, json_build_object('migrationNumber', $2::int, 'breaking', $3::boolean)::text)",
            vec![
                DbValue::Text(MIGRATION_CHANNEL.to_owned()),
                DbValue::I32(revision as i32),
                DbValue::Bool(migration.is_breaking()),
            ].into(),
        ).await?;
        Ok(())
    }.await;
    let Err(error) = tx.finish(result).await? else {
        return Ok(());
    };
    if let Err(hook_error) = run
        .emit(
            schema,
            MigrationPhase::RevisionFailed {
                revision,
                error: &error,
            },
        )
        .await
    {
        warn!(revision, %error, %hook_error, "Migration failure callback failed");
        return Err(hook_error.into());
    }
    if !inserted && is_clash_error(&error) {
        info!(
            revision,
            "Some other migration runner performed this migration; continuing."
        );
        return Ok(());
    }
    if executing_migration {
        return check_migration_error(revision, Err(error));
    }
    Err(error.into())
}

/// Verifies a running worker's schema without installing or updating it.
pub async fn check_running_schema(
    database: &Database,
    schema: &Schema,
) -> Result<(), MigrateError> {
    let last = timeout_at(Instant::now() + Duration::from_secs(2), async {
        let tx = database.begin().await?;
        tx.execute("SET LOCAL statement_timeout = '1s'", DbParams::new())
            .await?;
        let last = read_last_migration(&tx, schema).await?;
        tx.commit().await?;
        Ok::<_, DbError>(last)
    })
    .await
    .map_err(|error| DbError::new(format!("Running schema check: {error}")))??;
    let highest_migration = GRAPHILE_WORKER_MIGRATIONS
        .last()
        .map_or(0, |migration| migration.migration_number());
    check_compatible_revision(last.id, last.biggest_breaking_id, highest_migration)
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
