use graphile_worker_database::{DbError, DbParams, DbValue, Schema};
use indoc::formatdoc;
use tracing::info;

use crate::connection::MigrationConnection;

/// Installs the Graphile Worker schema into the database.
pub(super) async fn install_schema(
    connection: &MigrationConnection<'_>,
    schema: &Schema,
    version: u32,
) -> Result<(), DbError> {
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

    let tx = connection.begin().await?;
    let executor = tx.executor();
    let result = async {
        executor
            .execute(&create_schema_query, DbParams::new())
            .await?;
        executor
            .execute(&create_migration_table_query, DbParams::new())
            .await?;
        Ok(())
    }
    .await;
    tx.finish(result).await?
}

/// Annotates the migration table from releases that recorded revision IDs only.
pub(super) async fn add_breaking_migration_metadata(
    connection: &MigrationConnection<'_>,
    schema: &Schema,
) -> Result<(), DbError> {
    let migrations = schema.identifier("migrations");
    let breaking_ids = crate::sql::GRAPHILE_WORKER_MIGRATIONS
        .iter()
        .filter(|migration| migration.is_breaking())
        .map(|migration| migration.migration_number() as i32)
        .collect();
    let tx = connection.begin().await?;
    let executor = tx.executor();
    let result = async {
        executor.execute(
            &format!("alter table {migrations} add column if not exists breaking boolean not null default false"),
            DbParams::new(),
        ).await?;
        executor.execute(
            &format!("update {migrations} set breaking = true where id = any($1::int[]) and not breaking"),
            vec![DbValue::I32Array(breaking_ids)].into(),
        ).await?;
        Ok(())
    }.await;
    tx.finish(result).await?
}
