use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::Result;
use graphile_worker_job::Job;

use super::shared::FailJobTables;

pub struct FailedJob<'a> {
    pub job: &'a Job,
    pub error: &'a str,
}

#[tracing::instrument(skip_all, err, fields(otel.kind="client", db.system="postgresql"))]
pub async fn fail_jobs(
    mut executor: impl DbExecutorArg,
    jobs: &[FailedJob<'_>],
    schema: &Schema,
    worker_id: &str,
) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }

    let tables = FailJobTables::new(schema);
    let job_ids: Vec<i64> = jobs.iter().map(|job| *job.job.id()).collect();
    let errors: Vec<String> = jobs.iter().map(|job| job.error.to_string()).collect();
    let has_queues = jobs.iter().any(|job| job.job.job_queue_id().is_some());

    if has_queues {
        return fail_queued_jobs(&mut executor, &tables, worker_id, job_ids, errors).await;
    }

    fail_unqueued_jobs(&mut executor, &tables, worker_id, job_ids, errors).await
}

async fn fail_unqueued_jobs(
    executor: &mut impl DbExecutorArg,
    tables: &FailJobTables,
    worker_id: &str,
    job_ids: Vec<i64>,
    errors: Vec<String>,
) -> Result<()> {
    let jobs = &tables.jobs;
    let sql = formatdoc!(
        r#"
            WITH input AS (
                SELECT *
                FROM unnest($1::bigint[], $2::text[]) AS input(id, error)
            )
            UPDATE {jobs} AS jobs
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
        .execute(&sql, batch_params(worker_id, job_ids, errors).into())
        .await?;

    Ok(())
}

async fn fail_queued_jobs(
    executor: &mut impl DbExecutorArg,
    tables: &FailJobTables,
    worker_id: &str,
    job_ids: Vec<i64>,
    errors: Vec<String>,
) -> Result<()> {
    let jobs = &tables.jobs;
    let job_queues = &tables.job_queues;
    let sql = formatdoc!(
        r#"
            WITH input AS (
                SELECT *
                FROM unnest($1::bigint[], $2::text[]) AS input(id, error)
            ), j AS (
                UPDATE {jobs} AS jobs
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
            UPDATE {job_queues} AS job_queues
            SET locked_by = NULL, locked_at = NULL
            FROM j
            WHERE job_queues.id = j.job_queue_id
                AND job_queues.locked_by = $3::text;
        "#
    );

    executor
        .execute(&sql, batch_params(worker_id, job_ids, errors).into())
        .await?;

    Ok(())
}

fn batch_params(worker_id: &str, job_ids: Vec<i64>, errors: Vec<String>) -> Vec<DbValue> {
    vec![
        DbValue::I64Array(job_ids),
        DbValue::TextArray(errors),
        DbValue::Text(worker_id.to_string()),
    ]
}
