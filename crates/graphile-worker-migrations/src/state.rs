use std::time::Duration;

use graphile_worker_database::{Database, DbError, DbExecutor, DbParams, Schema};
use graphile_worker_runtime::sleep;
use indoc::formatdoc;
use tracing::info;

use crate::clash::is_clash_error;
use crate::error::MigrateError;
use crate::install::install_schema;

const INSTALL_RETRY_DELAYS: &[Duration] = &[
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
];

#[derive(Debug)]
pub(super) struct LastMigration {
    pub(super) server_version_num: String,
    pub(super) id: Option<i32>,
    pub(super) biggest_breaking_id: Option<i32>,
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

impl LastMigration {
    pub(super) fn is_before_number(&self, migration_number: u32) -> bool {
        let migration_id = self.id.and_then(|id| id.try_into().ok());
        self.id.is_none() || migration_number > migration_id.unwrap_or(0)
    }
}

/// Returns the last migration that was run against the database.
/// It also installs the Graphile Worker schema if it doesn't exist.
pub(super) async fn get_last_migration(
    executor: &Database,
    schema: &Schema,
) -> Result<LastMigration, MigrateError> {
    let migrations = schema.identifier("migrations");
    let migrations_status_query = formatdoc!(
        r#"
            select current_setting('server_version_num') as server_version_num,
            (select id from {migrations} order by id desc limit 1) as id,
            (select id from {migrations} where breaking is true order by id desc limit 1) as biggest_breaking_id;
        "#
    );
    for retry_delay in INSTALL_RETRY_DELAYS
        .iter()
        .copied()
        .map(Some)
        .chain(std::iter::once(None))
    {
        let last_migration_query_result =
            fetch_last_migration(executor, &migrations_status_query).await;
        let error = match last_migration_query_result {
            Ok(last_migration) => return Ok(last_migration),
            Err(error) => error,
        };

        if error.code() != Some("42P01") {
            return Err(MigrateError::SqlError(error));
        }

        info!(error = ?error, schema = %schema, "Graphile Worker schema not found, installing...");
        match install_schema(executor, schema).await {
            Ok(()) => return Ok(Default::default()),
            Err(MigrateError::SqlError(error)) if is_clash_error(&error) => {
                let Some(delay) = retry_delay else {
                    return Err(MigrateError::SqlError(error));
                };

                info!(
                    error = ?error,
                    schema = %schema,
                    "Graphile Worker schema was installed concurrently, retrying migration state read..."
                );
                sleep(delay).await;
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("install retry loop must return on the final attempt")
}

async fn fetch_last_migration(
    executor: &Database,
    migrations_status_query: &str,
) -> Result<LastMigration, DbError> {
    executor
        .fetch_one(migrations_status_query, DbParams::new())
        .await
        .and_then(|row| {
            Ok(LastMigration {
                server_version_num: row.try_get("server_version_num")?,
                id: row.try_get("id")?,
                biggest_breaking_id: row.try_get("biggest_breaking_id")?,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
