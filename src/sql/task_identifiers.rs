use graphile_worker_database::{DbExecutor, DbValue};
use indoc::formatdoc;

use crate::errors::Result;
pub use graphile_worker_task_details::{SharedTaskDetails, TaskDetails};

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
pub async fn get_tasks_details(
    executor: &impl DbExecutor,
    escaped_schema: &str,
    task_names: Vec<String>,
) -> Result<TaskDetails> {
    if task_names.is_empty() {
        return Ok(TaskDetails::new());
    }

    let insert_tasks_query = format!("insert into {escaped_schema}._private_tasks as tasks (identifier) select unnest($1::text[]) on conflict do nothing");
    executor
        .execute(
            &insert_tasks_query,
            vec![DbValue::TextArray(task_names.clone())].into(),
        )
        .await?;

    let select_tasks_query = formatdoc!(
        "select id, identifier from {escaped_schema}._private_tasks as tasks where identifier = any($1::text[])"
    );
    let tasks: Vec<TaskRow> = executor
        .fetch_all(
            &select_tasks_query,
            vec![DbValue::TextArray(task_names)].into(),
        )
        .await?
        .into_iter()
        .map(|row| {
            Ok(TaskRow {
                id: row.try_get("id")?,
                identifier: row.try_get("identifier")?,
            })
        })
        .collect::<std::result::Result<_, graphile_worker_database::DbError>>()?;

    Ok(task_rows_to_details(tasks))
}
