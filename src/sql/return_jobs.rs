use indoc::formatdoc;
use sqlx::{query, PgExecutor};

use crate::errors::Result;
use graphile_worker_job::Job;

pub async fn return_jobs<'e>(
    executor: impl PgExecutor<'e>,
    jobs: &[Job],
    escaped_schema: &str,
    worker_id: &str,
) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }

    let job_ids: Vec<i64> = jobs.iter().map(|j| *j.id()).collect();
    let queue_job_ids: Vec<i64> = jobs
        .iter()
        .filter(|j| j.job_queue_id().is_some())
        .map(|j| *j.id())
        .collect();

    let sql = formatdoc!(
        r#"
            with j as (
                update {escaped_schema}._private_jobs as jobs
                set
                    attempts = GREATEST(0, attempts - 1),
                    locked_by = null,
                    locked_at = null
                where id = ANY($2::bigint[])
                and locked_by = $1::text
                returning *
            )
            update {escaped_schema}._private_job_queues as job_queues
            set locked_by = null, locked_at = null
            from j
            where job_queues.id = j.job_queue_id
            and job_queues.locked_by = $1::text
            and j.id = ANY($3::bigint[]);
        "#
    );

    query(&sql)
        .bind(worker_id)
        .bind(&job_ids)
        .bind(&queue_job_ids)
        .execute(executor)
        .await?;

    Ok(())
}
