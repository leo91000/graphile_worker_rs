mod m000001;
mod m000002;
mod m000003;
mod m000004;
mod m000005;
mod m000006;
mod m000007;
mod m000008;
mod m000009;
mod m000010;
mod m000011;
mod m000012;
mod m000013;

use crate::errors::Result;
use sqlx::{query, Acquire, Error as SqlxError, PgExecutor, Postgres, Row};
use tracing::info;

use m000001::M000001_MIGRATION;
use m000002::M000002_MIGRATION;
use m000003::M000003_MIGRATION;
use m000004::M000004_MIGRATION;
use m000005::M000005_MIGRATION;
use m000006::M000006_MIGRATION;
use m000007::M000007_MIGRATION;
use m000008::M000008_MIGRATION;
use m000009::M000009_MIGRATION;
use m000010::M000010_MIGRATION;
use m000011::M000011_MIGRATION;
use m000012::M000012_MIGRATION;
use m000013::M000013_MIGRATION;

pub const MIGRATIONS: &[&[&str]] = &[
    M000001_MIGRATION,
    M000002_MIGRATION,
    M000003_MIGRATION,
    M000004_MIGRATION,
    M000005_MIGRATION,
    M000006_MIGRATION,
    M000007_MIGRATION,
    M000008_MIGRATION,
    M000009_MIGRATION,
    M000010_MIGRATION,
    M000011_MIGRATION,
    M000012_MIGRATION,
    M000013_MIGRATION,
];

async fn install_schema<'e, E>(executor: E, escaped_schema: &str) -> Result<()>
where
    E: PgExecutor<'e> + Acquire<'e, Database = Postgres> + Clone,
{
    let create_schema_query = format!(
        r#"
            create schema {escaped_schema};
        "#
    );

    let create_migration_table_query = format!(
        r#"
            create table {escaped_schema}.migrations (
                id int primary key, 
                ts timestamptz default now() not null
            );
        "#
    );

    let mut tx = executor.begin().await?;
    query(&create_schema_query).execute(&mut tx).await?;
    query(&create_migration_table_query)
        .execute(&mut tx)
        .await?;
    tx.commit().await?;

    Ok(())
}

pub async fn migrate<'e, E>(executor: E, escaped_schema: &str) -> Result<()>
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
            let Some(code) = e.code() else { return Err(SqlxError::Database(e).into()); };

            if code == "42P01" {
                install_schema(executor.clone(), escaped_schema).await?;
            } else {
                return Err(SqlxError::Database(e).into());
            }

            None
        }
        Err(e) => {
            return Err(e.into());
        }
        Ok(optional_row) => optional_row.map(|row| row.get("id")),
    };

    for (i, migration_statements) in MIGRATIONS.iter().enumerate() {
        let migration_number = (i + 1) as i32;

        if last_migration.is_none() || migration_number > last_migration.unwrap() {
            info!(migration_number, "Executing migration");
            let mut tx = executor.clone().begin().await?;

            for migration_statement in migration_statements.iter() {
                let sql = migration_statement.replace(":ARCHIMEDES_SCHEMA", escaped_schema);
                query(sql.as_str()).execute(&mut tx).await?;
            }

            query(format!("insert into {escaped_schema}.migrations (id) values ($1)").as_str())
                .bind(migration_number)
                .execute(&mut tx)
                .await?;

            tx.commit().await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::migrate::migrate;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn test_migrate() {
        let pool = &PgPoolOptions::new()
            .max_connections(5)
            .connect("postgres://postgres:root@localhost:5432")
            .await
            .expect("Failed to connect to database");

        migrate(pool, "test_migrate_3")
            .await
            .expect("Failed to migrate");
    }
}
