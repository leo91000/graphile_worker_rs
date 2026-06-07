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
