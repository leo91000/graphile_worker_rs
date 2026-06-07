use std::collections::{BTreeSet, HashMap};

use graphile_worker_database::Schema;
use indoc::formatdoc;
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::super::error::Result;
use crate::sql::AdminTable;

pub async fn task_identifiers_by_id(
    pool: &PgPool,
    schema: &Schema,
    task_ids: impl IntoIterator<Item = i32>,
) -> Result<HashMap<i32, String>> {
    let task_ids = task_ids.into_iter().collect::<BTreeSet<_>>();
    if task_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let tasks = AdminTable::Tasks.qualified(schema);
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select id, identifier
            from {tasks}
            where id in (
        "#
    ));
    let mut separated = query.separated(", ");
    for task_id in task_ids {
        separated.push_bind(task_id);
    }
    separated.push_unseparated(")");

    Ok(query
        .build_query_as::<(i32, String)>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests {
    use sqlx::{PgPool, Row};

    use super::*;

    async fn test_pool() -> Option<PgPool> {
        let database_url = std::env::var("DATABASE_URL").ok()?;
        PgPool::connect(&database_url).await.ok()
    }

    #[tokio::test]
    async fn task_identifiers_by_id_returns_empty_map_for_empty_input() {
        let Some(pool) = test_pool().await else {
            return;
        };

        let tasks = task_identifiers_by_id(&pool, &Schema::new("graphile_worker"), [])
            .await
            .expect("empty task lookup should succeed");

        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn task_identifiers_by_id_deduplicates_and_fetches_tasks() {
        let Some(pool) = test_pool().await else {
            return;
        };
        let schema = format!(
            "admin_api_tasks_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );

        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&pool)
            .await
            .expect("failed to create test schema");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TABLE {schema}._private_tasks (id integer primary key, identifier text not null)"
        )))
        .execute(&pool)
        .await
        .expect("failed to create task table");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {schema}._private_tasks (id, identifier) VALUES (1, 'send_email'), (2, 'send_sms')"
        )))
        .execute(&pool)
        .await
        .expect("failed to insert tasks");

        let tasks = task_identifiers_by_id(&pool, &Schema::new(&schema), [1, 1, 2])
            .await
            .expect("task lookup should succeed");

        assert_eq!(tasks.get(&1).map(String::as_str), Some("send_email"));
        assert_eq!(tasks.get(&2).map(String::as_str), Some("send_sms"));

        let count: i64 = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {schema}._private_tasks"
        )))
        .fetch_one(&pool)
        .await
        .expect("failed to count tasks")
        .get(0);
        assert_eq!(count, 2);

        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await
            .expect("failed to drop test schema");
    }
}
