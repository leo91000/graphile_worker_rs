use indoc::formatdoc;
use sqlx::{query, PgExecutor};

use crate::errors::GraphileWorkerError;

use crate::Job;

pub async fn fail_job(
    executor: impl for<'e> PgExecutor<'e>,
    job: &Job,
    escaped_schema: &str,
    worker_id: &str,
    message: &str,
    replacement_payload: Option<Vec<String>>,
) -> Result<(), GraphileWorkerError> {
    let replacement_payload = replacement_payload.and_then(|v| serde_json::to_string(&v).ok());

    if job.job_queue_id().is_some() {
        let sql = formatdoc!(
            r#"
                with j as (
                    update {escaped_schema}._private_jobs as jobs
                        set
                            last_error = $2::text,
                            run_at = greatest(now(), run_at) + (exp(least(attempts, 10)) * interval '1 second'),
                            locked_by = null,
                            locked_at = null,
                            payload = coalesce($4::json, jobs.payload)
                        where id = $1::bigint and locked_by = $3::text
                        returning *
                )
                update {escaped_schema}._private_job_queues as job_queues
                    set locked_by = null, locked_at = null
                    from j
                    where job_queues.id = j.job_queue_id and job_queues.locked_by = $3::text;
            "#
        );

        query(&sql)
            .bind(job.id())
            .bind(message)
            .bind(worker_id)
            .bind(replacement_payload)
            .execute(executor)
            .await?;
    } else {
        let sql = format!(
            r#"
                update {escaped_schema}._private_jobs as jobs
                    set
                        last_error = $2::text,
                        run_at = greatest(now(), run_at) + (exp(least(attempts, 10)) * interval '1 second'),
                        locked_by = null,
                        locked_at = null,
                        payload = coalesce($4::json, jobs.payload)
                    where id = $1::bigint and locked_by = $3::text;
            "#
        );

        query(&sql)
            .bind(job.id())
            .bind(message)
            .bind(worker_id)
            .bind(replacement_payload)
            .execute(executor)
            .await?;
    }

    Ok(())
}

#[allow(dead_code)]
pub async fn fail_jobs(
    executor: impl for<'e> PgExecutor<'e>,
    jobs: &[Job],
    escaped_schema: &str,
    worker_id: &str,
    message: &str,
) -> Result<(), GraphileWorkerError> {
    let sql = format!(
        r#"
            with j as (
                update {escaped_schema}._private_jobs as jobs
                    set
                        last_error = $2::text,
                        run_at = greatest(now(), run_at) + (exp(least(attempts, 10)) * interval '1 second'),
                        locked_by = null,
                        locked_at = null
                    where id = any($1::int[]) and locked_by = any($3::text[])
                    returning *
            ), queues as (
                update {escaped_schema}._private_job_queues as job_queues
                    set locked_by = null, locked_at = null
                    from j
                    where job_queues.id = j.job_queue_id and job_queues.locked_by = any($3::text[])
            )
            select * from j;
        "#
    );

    let job_ids: Vec<i64> = jobs.iter().map(|job| job.id()).copied().collect();

    query(&sql)
        .bind(job_ids)
        .bind(message)
        .bind(worker_id)
        .execute(executor)
        .await?;

    Ok(())
}
