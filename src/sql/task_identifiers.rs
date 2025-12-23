use std::collections::HashMap;

use indoc::formatdoc;
use sqlx::{query, query_as, FromRow, PgExecutor};
use tracing::warn;

use crate::errors::Result;

#[derive(Debug, Clone)]
pub struct TaskDetails(HashMap<i32, String>);

impl TaskDetails {
    pub fn task_ids(&self) -> Vec<i32> {
        self.0.keys().copied().collect()
    }

    pub fn task_names(&self) -> Vec<String> {
        self.0.values().cloned().collect()
    }

    pub fn get(&self, id: &i32) -> Option<&String> {
        self.0.get(id)
    }

    pub fn get_identifier(&self, job_id: &i64, task_id: &i32) -> String {
        match self.0.get(task_id) {
            Some(identifier) => identifier.to_owned(),
            None => {
                warn!(
                    job_id,
                    task_id,
                    "Unknown task_id for job, using empty task identifier"
                );
                String::new()
            }
        }
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

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
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

    let select_tasks_query = formatdoc!(
        "select id, identifier from {escaped_schema}._private_tasks as tasks where identifier = any($1::text[])"
    );
    let tasks: Vec<TaskRow> = query_as(&select_tasks_query)
        .bind(&task_names)
        .fetch_all(executor)
        .await?;

    Ok(tasks.into())
}
