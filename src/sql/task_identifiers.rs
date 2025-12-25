use indoc::formatdoc;
use sqlx::{query, query_as, FromRow, PgExecutor};

use crate::errors::Result;
pub use graphile_worker_task_details::{SharedTaskDetails, TaskDetails};

#[derive(FromRow)]
struct TaskRow {
    id: i32,
    identifier: String,
}

fn task_rows_to_details(tasks: Vec<TaskRow>) -> TaskDetails {
    let mut details = TaskDetails::new();
    for row in tasks {
        details.insert(row.id, row.identifier);
    }
    details
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

    Ok(task_rows_to_details(tasks))
}
