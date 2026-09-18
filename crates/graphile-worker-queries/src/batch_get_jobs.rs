use chrono::{DateTime, Utc};
use graphile_worker_database::{DbExecutorArg, DbParams, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::Result;
use graphile_worker_job::Job;

use super::job_query_helpers::{
    get_flag_clause, get_now_clause, get_queue_clause, get_update_queue_clause,
};
use super::task_identifiers::TaskDetails;

/// Claims eligible jobs and their named queues for one worker.
///
/// Candidates are locked before selecting at most one job per named queue;
/// unnamed jobs remain independent. The candidate limit can therefore yield
/// fewer than `batch_size` jobs. `now = None` uses PostgreSQL time.
pub async fn batch_get_jobs(
    mut executor: impl DbExecutorArg,
    task_details: &TaskDetails,
    schema: impl Into<Schema>,
    worker_id: &str,
    flags_to_skip: &[String],
    batch_size: i32,
    now: Option<DateTime<Utc>>,
) -> Result<Vec<Job>> {
    let schema = schema.into();
    let has_flags = !flags_to_skip.is_empty();
    let has_now = now.is_some();

    let mut next_param: u8 = 4;
    let flag_param = if has_flags {
        let p = next_param;
        next_param += 1;
        Some(p)
    } else {
        None
    };
    let now_param = if has_now { Some(next_param) } else { None };

    let sql = super::fetch_query_cache::fetch_query(&schema, has_flags, has_now, true, || {
        let flag_clause = flag_param
            .map(|p| get_flag_clause(flags_to_skip, p))
            .unwrap_or_default();
        let jobs = schema.private_table("jobs");
        let queue_clause = get_queue_clause(&schema);
        let update_queue_clause = get_update_queue_clause(&schema, 1, now_param);
        let now_clause = get_now_clause(now_param);

        formatdoc!(
            r#"
            with j_raw as (
                select jobs.job_queue_id, jobs.priority, jobs.run_at, jobs.id
                    from {jobs} as jobs
                    where jobs.is_available = true
                    and run_at <= {now_clause}
                    and task_id = any($2::int[])
                    {queue_clause}
                    {flag_clause}
                    order by priority asc, run_at asc
                    limit $3::int
                    for update
                    skip locked
                ), j as (
                    select distinct on (job_queue_id, case when job_queue_id is null then id end)
                        job_queue_id, priority, run_at, id
                    from j_raw
                    order by job_queue_id, case when job_queue_id is null then id end,
                        priority asc, run_at asc, id asc
                ) {update_queue_clause}
                    update {jobs} as jobs
                        set
                            attempts = jobs.attempts + 1,
                            locked_by = $1::text,
                            locked_at = {now_clause}
                        from j
                        where jobs.id = j.id
                        returning *
        "#
        )
    });

    let mut params = vec![
        DbValue::Text(worker_id.to_string()),
        DbValue::I32Array(task_details.task_ids().to_vec()),
        DbValue::I32(batch_size),
    ];

    if has_flags {
        params.push(DbValue::TextArray(flags_to_skip.to_vec()));
    }
    if let Some(ts) = now {
        params.push(DbValue::TimestampTz(ts));
    }

    let jobs = executor
        .fetch_all(&sql, DbParams::from(params))
        .await?
        .into_iter()
        .map(|row| super::rows::db_job_from_row(&row))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(jobs
        .into_iter()
        .map(|job| {
            let task_identifier = task_details.get_or_empty(job.id(), job.task_id());
            Job::from_db_job(job, task_identifier)
        })
        .collect())
}
