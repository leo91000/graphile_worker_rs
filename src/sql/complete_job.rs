use graphile_worker_database::{DbExecutorArg, DbValue};
use indoc::formatdoc;

use crate::errors::GraphileWorkerError;

use crate::Job;

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn complete_job(
    mut executor: impl DbExecutorArg,
    job: &Job,
    worker_id: &str,
    escaped_schema: &str,
) -> Result<(), GraphileWorkerError> {
    if job.job_queue_id().is_some() {
        let sql = formatdoc!(
            r#"
                with j as (
                    delete from {escaped_schema}._private_jobs as jobs
                    where id = $1::bigint
                    returning *
                )
                update {escaped_schema}._private_job_queues as job_queues
                    set locked_by = null, locked_at = null
                    from j
                    where job_queues.id = j.job_queue_id and job_queues.locked_by = $2::text;
            "#
        );

        executor
            .execute(
                &sql,
                vec![
                    DbValue::I64(*job.id()),
                    DbValue::Text(worker_id.to_string()),
                ]
                .into(),
            )
            .await?;
    } else {
        let sql = format!(
            r#"
                delete from {escaped_schema}._private_jobs
                where id = $1::bigint;
            "#
        );

        executor
            .execute(&sql, vec![DbValue::I64(*job.id())].into())
            .await?;
    }

    Ok(())
}
