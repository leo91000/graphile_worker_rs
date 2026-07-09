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
    let mut retry_delays = INSTALL_RETRY_DELAYS
        .iter()
        .copied()
        .map(Some)
        .chain(std::iter::once(None));

    loop {
        let retry_delay = retry_delays.next().flatten();
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
    use std::any::Any;
    use std::collections::VecDeque;
    use std::future::Future;
    use std::sync::{Arc, Mutex};

    use graphile_worker_database::{
        row_mapping, BoxFuture, DatabaseDriver, DbCell, DbRow, DbTransaction, NotificationStream,
        TransactionDriver,
    };

    use super::*;

    type FetchResult = Result<Vec<DbRow>, DbError>;
    type FetchResults = Arc<Mutex<VecDeque<FetchResult>>>;

    #[derive(Debug)]
    struct MockDriver {
        fetch_results: FetchResults,
        transaction_execute_result: Result<u64, DbError>,
    }

    impl MockDriver {
        fn new(
            fetch_results: impl IntoIterator<Item = Result<Vec<DbRow>, DbError>>,
            transaction_execute_result: Result<u64, DbError>,
        ) -> Self {
            Self {
                fetch_results: Arc::new(Mutex::new(fetch_results.into_iter().collect())),
                transaction_execute_result,
            }
        }
    }

    impl DbExecutor for MockDriver {
        fn execute<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<u64, DbError>> {
            Box::pin(async { Ok(1) })
        }

        fn fetch_all<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
            Box::pin(async {
                self.fetch_results
                    .lock()
                    .expect("mock fetch result lock should not be poisoned")
                    .pop_front()
                    .expect("mock fetch result should be configured")
            })
        }
    }

    impl DatabaseDriver for MockDriver {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn begin<'a>(&'a self) -> BoxFuture<'a, Result<DbTransaction, DbError>> {
            Box::pin(async {
                Ok(DbTransaction::new(Box::new(MockTransaction {
                    execute_result: self.transaction_execute_result.clone(),
                })))
            })
        }

        fn listen<'a>(
            &'a self,
            _channel: &'a str,
        ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
            Box::pin(async { Ok(None) })
        }
    }

    #[derive(Debug)]
    struct MockTransaction {
        execute_result: Result<u64, DbError>,
    }

    impl DbExecutor for MockTransaction {
        fn execute<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<u64, DbError>> {
            Box::pin(async { self.execute_result.clone() })
        }

        fn fetch_all<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    impl TransactionDriver for MockTransaction {
        fn commit(self: Box<Self>) -> BoxFuture<'static, Result<(), DbError>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn database(
        fetch_results: impl IntoIterator<Item = Result<Vec<DbRow>, DbError>>,
        transaction_execute_result: Result<u64, DbError>,
    ) -> Database {
        Database::new(MockDriver::new(fetch_results, transaction_execute_result))
    }

    fn server_version_row(version: &str) -> DbRow {
        row_mapping::cells([("server_version_num", DbCell::Text(version.to_string()))])
    }

    fn run_async<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime should build")
            .block_on(future)
    }

    #[test]
    fn mock_driver_exercises_database_contract_paths() {
        run_async(async {
            let database = database([Ok(Vec::new())], Ok(7));

            assert!(database.downcast_ref::<MockDriver>().is_some());
            assert_eq!(
                database
                    .execute("select 1", DbParams::new())
                    .await
                    .expect("mock execute should succeed"),
                1
            );
            assert!(database
                .listen("jobs:insert")
                .await
                .expect("mock listen should succeed")
                .is_none());

            let transaction = database.begin().await.expect("mock begin should succeed");
            assert_eq!(
                transaction
                    .execute("select 1", DbParams::new())
                    .await
                    .expect("mock transaction execute should succeed"),
                7
            );
            assert!(transaction
                .fetch_all("select 1", DbParams::new())
                .await
                .expect("mock transaction fetch should succeed")
                .is_empty());
            transaction
                .commit()
                .await
                .expect("mock transaction commit should succeed");
        });
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
    fn get_last_migration_keeps_non_missing_schema_errors() {
        run_async(async {
            let database = database(
                [Err(DbError::with_code("permission denied", "42501"))],
                Ok(1),
            );

            let error = get_last_migration(&database, &Schema::default())
                .await
                .expect_err("non-missing schema errors should be returned");

            assert!(matches!(
                error,
                MigrateError::SqlError(ref error) if error.code() == Some("42501")
            ));
        });
    }

    #[test]
    fn get_last_migration_keeps_install_errors() {
        run_async(async {
            let database = database(
                [
                    Err(DbError::with_code("missing migrations table", "42P01")),
                    Ok(vec![server_version_row("110000")]),
                ],
                Ok(1),
            );

            let error = get_last_migration(&database, &Schema::default())
                .await
                .expect_err("install errors should be returned");

            assert!(matches!(error, MigrateError::IncompatibleVersion(110000)));
        });
    }

    #[test]
    fn get_last_migration_stops_after_install_clash_retries() {
        run_async(async {
            let mut fetch_results = Vec::new();
            for _ in 0..=INSTALL_RETRY_DELAYS.len() {
                fetch_results.push(Err(DbError::with_code("missing migrations table", "42P01")));
                fetch_results.push(Ok(vec![server_version_row("120000")]));
            }
            let database = database(
                fetch_results,
                Err(DbError::with_code("concurrent install", "23505")),
            );

            let error = get_last_migration(&database, &Schema::default())
                .await
                .expect_err("install clash retries should be bounded");

            assert!(matches!(
                error,
                MigrateError::SqlError(ref error) if error.code() == Some("23505")
            ));
        });
    }
}
