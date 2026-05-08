#[cfg(feature = "driver-sqlx")]
use graphile_worker_database::DbError;
use graphile_worker_database::{DbExecutor, DbValue};
use indoc::formatdoc;

use crate::errors::Result;

use crate::Job;

pub struct FailedJob<'a> {
    pub job: &'a Job,
    pub error: &'a str,
}

#[cfg(feature = "driver-sqlx")]
async fn fail_jobs_sqlx(
    pool: &sqlx::PgPool,
    jobs: &[FailedJob<'_>],
    escaped_schema: &str,
    worker_id: &str,
) -> Result<()> {
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

        sqlx::query(&sql)
            .bind(&job_ids)
            .bind(&errors)
            .bind(worker_id)
            .execute(pool)
            .await
            .map_err(DbError::from)?;
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

        sqlx::query(&sql)
            .bind(&job_ids)
            .bind(&errors)
            .bind(worker_id)
            .execute(pool)
            .await
            .map_err(DbError::from)?;
    }

    Ok(())
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn fail_job(
    executor: &impl DbExecutor,
    job: &Job,
    escaped_schema: &str,
    worker_id: &str,
    message: &str,
    replacement_payload: Option<Vec<String>>,
) -> Result<()> {
    let replacement_payload = replacement_payload.map(|value| serde_json::json!(value));

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

        executor
            .execute(
                &sql,
                vec![
                    DbValue::I64(*job.id()),
                    DbValue::Text(message.to_string()),
                    DbValue::Text(worker_id.to_string()),
                    DbValue::JsonOpt(replacement_payload.clone()),
                ]
                .into(),
            )
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

        executor
            .execute(
                &sql,
                vec![
                    DbValue::I64(*job.id()),
                    DbValue::Text(message.to_string()),
                    DbValue::Text(worker_id.to_string()),
                    DbValue::JsonOpt(replacement_payload),
                ]
                .into(),
            )
            .await?;
    }

    Ok(())
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn fail_jobs(
    executor: &impl DbExecutor,
    jobs: &[FailedJob<'_>],
    escaped_schema: &str,
    worker_id: &str,
) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }

    #[cfg(feature = "driver-sqlx")]
    if let Some(pool) = executor.try_sqlx_pool() {
        return fail_jobs_sqlx(pool, jobs, escaped_schema, worker_id).await;
    }

    let job_ids: Vec<i64> = jobs.iter().map(|job| *job.job.id()).collect();
    let errors: Vec<String> = jobs.iter().map(|job| job.error.to_string()).collect();
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

        executor
            .execute(
                &sql,
                vec![
                    DbValue::I64Array(job_ids),
                    DbValue::TextArray(errors),
                    DbValue::Text(worker_id.to_string()),
                ]
                .into(),
            )
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

        executor
            .execute(
                &sql,
                vec![
                    DbValue::I64Array(job_ids),
                    DbValue::TextArray(errors),
                    DbValue::Text(worker_id.to_string()),
                ]
                .into(),
            )
            .await?;
    }

    Ok(())
}
