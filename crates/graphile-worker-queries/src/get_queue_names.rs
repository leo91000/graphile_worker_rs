use graphile_worker_database::{DbExecutorArg, DbValue, Schema};

use crate::errors::Result;

/// Resolves queue identities in input order, preserving duplicates and missing IDs.
///
/// Missing queues return `None`. No result cache is retained, so cleanup and
/// subsequent identity allocation cannot leave stale names in an application.
pub async fn get_queue_names(
    mut executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    queue_ids: &[i32],
) -> Result<Vec<Option<String>>> {
    if queue_ids.is_empty() {
        return Ok(Vec::new());
    }
    let schema = schema.into();
    let queues = schema.private_table("job_queues");
    let rows = executor
        .fetch_all(
            &format!("SELECT id, queue_name FROM {queues} WHERE id = ANY($1::int[])"),
            vec![DbValue::I32Array(queue_ids.to_vec())].into(),
        )
        .await?;
    let names =
        rows.iter()
            .map(|row| {
                Ok((
                    row.try_get::<i32>("id")?,
                    row.try_get::<String>("queue_name")?,
                ))
            })
            .collect::<std::result::Result<
                std::collections::HashMap<_, _>,
                graphile_worker_database::DbError,
            >>()?;
    Ok(queue_ids.iter().map(|id| names.get(id).cloned()).collect())
}
