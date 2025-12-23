use indoc::formatdoc;
use sqlx::{query_as, PgExecutor};

use crate::errors::Result;
use graphile_worker_job::{DbJob, Job};

use super::job_query_helpers::{get_flag_clause, get_queue_clause, get_update_queue_clause};
use super::task_identifiers::TaskDetails;

pub async fn batch_get_jobs<'e>(
    executor: impl PgExecutor<'e>,
    task_details: &TaskDetails,
    escaped_schema: &str,
    worker_id: &str,
    flags_to_skip: &[String],
    batch_size: i32,
) -> Result<Vec<Job>> {
    let flag_clause = get_flag_clause(flags_to_skip, 4);
    let queue_clause = get_queue_clause(escaped_schema);
    let update_queue_clause = get_update_queue_clause(escaped_schema, 1);

    let sql = formatdoc!(
        r#"
            with j as (
                select jobs.job_queue_id, jobs.priority, jobs.run_at, jobs.id
                    from {escaped_schema}._private_jobs as jobs
                    where jobs.is_available = true
                    and run_at <= now()
                    and task_id = any($2::int[])
                    {queue_clause}
                    {flag_clause}
                    order by priority asc, run_at asc
                    limit $3
                    for update
                    skip locked
                ) {update_queue_clause}
                    update {escaped_schema}._private_jobs as jobs
                        set
                            attempts = jobs.attempts + 1,
                            locked_by = $1::text,
                            locked_at = now()
                        from j
                        where jobs.id = j.id
                        returning *
        "#
    );

    let mut q = query_as(&sql)
        .bind(worker_id)
        .bind(task_details.task_ids())
        .bind(batch_size);

    if !flags_to_skip.is_empty() {
        q = q.bind(flags_to_skip);
    }

    let jobs: Vec<DbJob> = q.fetch_all(executor).await?;
    Ok(jobs
        .into_iter()
        .map(|job| {
            let task_identifier = task_details.get_identifier(job.id(), job.task_id());
            Job::from_db_job(job, task_identifier)
        })
        .collect())
}
