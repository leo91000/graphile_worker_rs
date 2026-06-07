use graphile_worker_database::{Database, DbExecutor, DbParams, Schema};
use indoc::formatdoc;
use tracing::info;

use crate::error::MigrateError;
use crate::pg_version::fetch_and_check_postgres_version;

/// Installs the Graphile Worker schema into the database.
pub(super) async fn install_schema(
    database: &Database,
    schema: &Schema,
) -> Result<(), MigrateError> {
    let version = fetch_and_check_postgres_version(database).await?;
    info!(pg_version = version, "Installing Graphile Worker schema");

    let create_schema_query = formatdoc!(
        r#"
            create schema if not exists {schema};
        "#
    );

    let migrations = schema.identifier("migrations");
    let create_migration_table_query = formatdoc!(
        r#"
            create table if not exists {migrations} (
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
