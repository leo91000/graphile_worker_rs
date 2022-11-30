use sqlx::{query, PgExecutor};

use crate::errors::ArchimedesError;

use super::get_job::Job;

pub async fn complete_job(
    executor: impl for<'e> PgExecutor<'e>,
    job: &Job,
    worker_id: &str,
    escaped_schema: &str,
) -> Result<(), ArchimedesError> {
    if job.job_queue_id().is_some() {
        let sql = format!(
            r#"
            with j as (
                delete from {escaped_schema}.jobs
                where id = $1
                returning *
            )
            update {escaped_schema}.job_queues
                set locked_by = null, locked_at = null
                from j
                where job_queues.id = j.job_queue_id and job_queues.locked_by = $2;
        "#
        );

        query(&sql)
            .bind(job.id())
            .bind(worker_id)
            .execute(executor)
            .await?;
    } else {
        let sql = format!(
            r#"
                delete from {escaped_schema}.jobs
                where id = $1
            "#
        );

        query(&sql).bind(job.id()).execute(executor).await?;
    }

    Ok(())
}
