use std::collections::HashMap;

use sqlx::{query, query_as, FromRow, PgExecutor};

use crate::errors::Result;

#[derive(Debug, Clone)]
pub struct TaskDetails(HashMap<i32, String>);

impl TaskDetails {
    pub fn task_ids(&self) -> Vec<i32> {
        self.0.keys().copied().collect()
    }

    pub fn get(&self, id: &i32) -> Option<&String> {
        self.0.get(id)
    }
}

#[derive(FromRow)]
struct TaskRow {
    id: i32,
    identifier: String,
}

impl From<Vec<TaskRow>> for TaskDetails {
    fn from(tasks: Vec<TaskRow>) -> Self {
        let mut details = HashMap::new();
        for row in tasks.iter() {
            details.insert(row.id, row.identifier.clone());
        }
        TaskDetails(details)
    }
}

pub async fn get_tasks_details<'e>(
    executor: impl PgExecutor<'e> + Clone,
    escaped_schema: &str,
    task_names: Vec<String>,
) -> Result<TaskDetails> {
    let insert_tasks_query = format!("insert into {escaped_schema}._private_tasks as tasks (identifier) select unnest($1::text[]) on conflict do nothing");
    query(&insert_tasks_query)
        .bind(&task_names)
        .execute(executor.clone())
        .await?;

    let select_tasks_query = format!(
        "select id, identifier from {escaped_schema}._private_tasks as tasks where identifier = any($1::text[])"
    );
    let tasks: Vec<TaskRow> = query_as(&select_tasks_query)
        .bind(&task_names)
        .fetch_all(executor)
        .await?;

    Ok(tasks.into())
}
