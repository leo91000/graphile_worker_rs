use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::GraphileWorkerError;

use graphile_worker_job::Job;

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn complete_job(
    mut executor: impl DbExecutorArg,
    job: &Job,
    worker_id: &str,
    schema: impl Into<Schema>,
) -> Result<(), GraphileWorkerError> {
    let schema = schema.into();
    let jobs = schema.private_table("jobs");
    let job_queues = schema.private_table("job_queues");

    if job.job_queue_id().is_some() {
        let sql = formatdoc!(
            r#"
                with j as (
                    delete from {jobs} as jobs
                    where id = $1::bigint
                    returning *
                )
                update {job_queues} as job_queues
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
                delete from {jobs}
                where id = $1::bigint;
            "#
        );

        executor
            .execute(&sql, vec![DbValue::I64(*job.id())].into())
            .await?;
    }

    Ok(())
}
