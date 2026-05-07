use indoc::formatdoc;
use sqlx::{query, PgExecutor};

use crate::errors::GraphileWorkerError;

use crate::Job;

pub struct FailedJob<'a> {
    pub job: &'a Job,
    pub error: &'a str,
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
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

pub async fn fail_jobs(
    executor: impl for<'e> PgExecutor<'e>,
    jobs: &[FailedJob<'_>],
    escaped_schema: &str,
    worker_id: &str,
) -> Result<(), GraphileWorkerError> {
    if jobs.is_empty() {
        return Ok(());
    }

    let job_ids: Vec<i64> = jobs.iter().map(|job| *job.job.id()).collect();
    let errors: Vec<&str> = jobs.iter().map(|job| job.error).collect();
    let has_queues = jobs.iter().any(|job| job.job.job_queue_id().is_some());

    if has_queues {
        let sql = formatdoc!(
            r#"
                WITH input AS (
                    SELECT *
                    FROM unnest($1::bigint[], $2::text[]) AS input(id, error)
                ), j AS (
                    UPDATE {escaped_schema}._private_jobs AS jobs
                    SET
                        last_error = input.error,
                        run_at = greatest(now(), jobs.run_at) + (exp(least(jobs.attempts, 10)) * interval '1 second'),
                        locked_by = NULL,
                        locked_at = NULL
                    FROM input
                    WHERE jobs.id = input.id
                        AND jobs.locked_by = $3::text
                    RETURNING jobs.job_queue_id
                )
                UPDATE {escaped_schema}._private_job_queues AS job_queues
                SET locked_by = NULL, locked_at = NULL
                FROM j
                WHERE job_queues.id = j.job_queue_id
                    AND job_queues.locked_by = $3::text;
            "#
        );

        query(&sql)
            .bind(&job_ids)
            .bind(&errors)
            .bind(worker_id)
            .execute(executor)
            .await?;
    } else {
        let sql = formatdoc!(
            r#"
                WITH input AS (
                    SELECT *
                    FROM unnest($1::bigint[], $2::text[]) AS input(id, error)
                )
                UPDATE {escaped_schema}._private_jobs AS jobs
                SET
                    last_error = input.error,
                    run_at = greatest(now(), jobs.run_at) + (exp(least(jobs.attempts, 10)) * interval '1 second'),
                    locked_by = NULL,
                    locked_at = NULL
                FROM input
                WHERE jobs.id = input.id
                    AND jobs.locked_by = $3::text;
            "#
        );

        query(&sql)
            .bind(&job_ids)
            .bind(&errors)
            .bind(worker_id)
            .execute(executor)
            .await?;
    }

    Ok(())
}
