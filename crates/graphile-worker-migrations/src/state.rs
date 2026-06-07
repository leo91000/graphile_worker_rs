use graphile_worker_database::{Database, DbExecutor, DbParams, Schema};
use indoc::formatdoc;
use tracing::info;

use crate::error::MigrateError;
use crate::install::install_schema;

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
                info!(error = ?e, schema = %schema, "Graphile Worker schema not found, installing...");
                install_schema(executor, schema).await?;
                return Ok(Default::default());
            }

            return Err(MigrateError::SqlError(e));
        }
        Ok(row) => row,
    };
    Ok(last_migration)
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
