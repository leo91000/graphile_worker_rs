use crate::errors::Result;
use sqlx::{query, Acquire, Error as SqlxError, Executor, Postgres, Row};
use tracing::info;

async fn install_schema<'e, E>(executor: E, escaped_schema: &str) -> Result<()>
where
    E: Executor<'e, Database = Postgres> + Acquire<'e, Database = Postgres> + Clone,
{
    println!("Installing archimedes schema");

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
    E: Executor<'e, Database = Postgres> + Acquire<'e, Database = Postgres> + Send + Sync + Clone,
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

    for (i, migration_statements) in crate::migrations::MIGRATIONS.iter().enumerate() {
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
