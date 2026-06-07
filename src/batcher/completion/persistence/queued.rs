use graphile_worker_database::{Database, DbExecutor, DbValue, Schema};
use indoc::formatdoc;
use tracing::error;

use super::super::CompletionRequest;

pub(super) async fn complete_jobs_with_queues(
    database: &Database,
    schema: &Schema,
    worker_id: &str,
    job_ids: Vec<i64>,
) -> bool {
    if job_ids.is_empty() {
        return false;
    }

    let jobs = schema.private_table("jobs");
    let job_queues = schema.private_table("job_queues");
    let sql = formatdoc!(
        r#"
            WITH j AS (
                DELETE FROM {jobs}
                WHERE id = ANY($1::bigint[])
                RETURNING *
            )
            UPDATE {job_queues} AS job_queues
            SET locked_by = NULL, locked_at = NULL
            FROM j
            WHERE job_queues.id = j.job_queue_id AND job_queues.locked_by = $2::text
        "#
    );

    match database
        .execute(
            &sql,
            vec![
                DbValue::I64Array(job_ids),
                DbValue::Text(worker_id.to_string()),
            ]
            .into(),
        )
        .await
    {
        Ok(_) => true,
        Err(error) => {
            error!(?error, "Failed to complete jobs with queue");
            false
        }
    }
}

pub(super) async fn complete_one_job_with_queue(
    req: &CompletionRequest,
    database: &Database,
    schema: &Schema,
    worker_id: &str,
) -> bool {
    let jobs = schema.private_table("jobs");
    let job_queues = schema.private_table("job_queues");
    let sql = formatdoc!(
        r#"
            WITH j AS (
                DELETE FROM {jobs}
                WHERE id = $1
                RETURNING *
            )
            UPDATE {job_queues} AS job_queues
            SET locked_by = NULL, locked_at = NULL
            FROM j
            WHERE job_queues.id = j.job_queue_id AND job_queues.locked_by = $2::text
        "#
    );

    if let Err(error) = database
        .execute(
            &sql,
            vec![
                DbValue::I64(req.job_id),
                DbValue::Text(worker_id.to_string()),
            ]
            .into(),
        )
        .await
    {
        error!(
            ?error,
            job_id = req.job_id,
            "Failed to complete job directly (with queue)"
        );
        return false;
    }

    true
}
