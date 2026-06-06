use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, DbValue};
use indoc::formatdoc;

use crate::errors::Result;
use crate::sql::duration::duration_as_millis_i64;
use graphile_worker_job::Job;

pub async fn return_jobs(
    mut executor: impl DbExecutorArg,
    jobs: &[Job],
    escaped_schema: &str,
    worker_id: &str,
) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }

    let mut job_ids = Vec::with_capacity(jobs.len());
    let mut queue_job_ids = Vec::new();
    for job in jobs {
        let id = *job.id();
        job_ids.push(id);
        if job.job_queue_id().is_some() {
            queue_job_ids.push(id);
        }
    }

    if queue_job_ids.is_empty() {
        let sql = formatdoc!(
            r#"
                update {escaped_schema}._private_jobs as jobs
                set
                    attempts = GREATEST(0, attempts - 1),
                    locked_by = null,
                    locked_at = null
                where id = ANY($2::bigint[])
                and locked_by = $1::text;
            "#
        );

        executor
            .execute(
                &sql,
                vec![
                    DbValue::Text(worker_id.to_string()),
                    DbValue::I64Array(job_ids),
                ]
                .into(),
            )
            .await?;
    } else {
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

        executor
            .execute(
                &sql,
                vec![
                    DbValue::Text(worker_id.to_string()),
                    DbValue::I64Array(job_ids),
                    DbValue::I64Array(queue_job_ids),
                ]
                .into(),
            )
            .await?;
    }

    Ok(())
}

pub async fn return_job_for_recovery(
    mut executor: impl DbExecutorArg,
    job: &Job,
    escaped_schema: &str,
    worker_id: &str,
    recovery_delay: Option<Duration>,
    last_error: Option<&str>,
) -> Result<()> {
    if job.job_queue_id().is_some() {
        let sql = formatdoc!(
            r#"
                with j as (
                    update {escaped_schema}._private_jobs as jobs
                    set
                        attempts = GREATEST(0, attempts - 1),
                        locked_by = null,
                        locked_at = null,
                        run_at = CASE
                            WHEN $3::bigint IS NULL THEN jobs.run_at
                            ELSE GREATEST(jobs.run_at, now() + ($3::bigint * interval '1 millisecond'))
                        END,
                        last_error = COALESCE($4::text, jobs.last_error),
                        updated_at = now()
                    where id = $2::bigint
                    and locked_by = $1::text
                    returning *
                )
                update {escaped_schema}._private_job_queues as job_queues
                set locked_by = null, locked_at = null
                from j
                where job_queues.id = j.job_queue_id
                and job_queues.locked_by = $1::text;
            "#
        );

        executor
            .execute(
                &sql,
                vec![
                    DbValue::Text(worker_id.to_string()),
                    DbValue::I64(*job.id()),
                    DbValue::I64Opt(recovery_delay.map(duration_as_millis_i64)),
                    DbValue::TextOpt(last_error.map(ToString::to_string)),
                ]
                .into(),
            )
            .await?;
    } else {
        let sql = formatdoc!(
            r#"
                update {escaped_schema}._private_jobs as jobs
                set
                        attempts = GREATEST(0, attempts - 1),
                        locked_by = null,
                        locked_at = null,
                        run_at = CASE
                            WHEN $3::bigint IS NULL THEN jobs.run_at
                            ELSE GREATEST(jobs.run_at, now() + ($3::bigint * interval '1 millisecond'))
                        END,
                        last_error = COALESCE($4::text, jobs.last_error),
                        updated_at = now()
                where id = $2::bigint
                and locked_by = $1::text;
            "#
        );

        executor
            .execute(
                &sql,
                vec![
                    DbValue::Text(worker_id.to_string()),
                    DbValue::I64(*job.id()),
                    DbValue::I64Opt(recovery_delay.map(duration_as_millis_i64)),
                    DbValue::TextOpt(last_error.map(ToString::to_string)),
                ]
                .into(),
            )
            .await?;
    }

    Ok(())
}
