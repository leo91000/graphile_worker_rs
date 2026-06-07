use graphile_worker_database::{Database, DbExecutor, DbValue, Schema};
use indoc::formatdoc;
use tracing::error;

use super::super::CompletionRequest;

pub(super) async fn complete_jobs_without_queues(
    database: &Database,
    schema: &Schema,
    job_ids: Vec<i64>,
) -> bool {
    if job_ids.is_empty() {
        return false;
    }

    let jobs = schema.private_table("jobs");
    let sql = formatdoc!(
        r#"
            DELETE FROM {jobs}
            WHERE id = ANY($1::bigint[])
        "#
    );

    match database
        .execute(&sql, vec![DbValue::I64Array(job_ids)].into())
        .await
    {
        Ok(_) => true,
        Err(error) => {
            error!(?error, "Failed to complete jobs without queue");
            false
        }
    }
}

pub(super) async fn complete_one_job_without_queue(
    req: &CompletionRequest,
    database: &Database,
    schema: &Schema,
) -> bool {
    let jobs = schema.private_table("jobs");
    let sql = formatdoc!(
        r#"
            DELETE FROM {jobs}
            WHERE id = $1
        "#
    );

    if let Err(error) = database
        .execute(&sql, vec![DbValue::I64(req.job_id)].into())
        .await
    {
        error!(
            ?error,
            job_id = req.job_id,
            "Failed to complete job directly"
        );
        return false;
    }

    true
}
