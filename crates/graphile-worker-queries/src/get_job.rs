use chrono::{DateTime, Utc};
use graphile_worker_database::{DbExecutorArg, DbParams, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::Result;
use crate::flag_filter::JobFlagFilter;
use graphile_worker_job::Job;

use super::job_query_helpers::{get_now_clause, get_queue_clause, get_update_queue_clause};
use super::task_identifiers::TaskDetails;

/// Claims the next eligible job, incrementing its attempts and locking its queue.
///
/// Only registered tasks and jobs without forbidden flags are considered.
/// An explicit `now` uses local time; otherwise PostgreSQL supplies the clock.
pub async fn get_job(
    executor: impl DbExecutorArg,
    task_details: &TaskDetails,
    schema: impl Into<Schema>,
    worker_id: &str,
    flags_to_skip: &[String],
    now: Option<DateTime<Utc>>,
) -> Result<Option<Job>> {
    get_job_with_filter(
        executor,
        task_details,
        schema,
        worker_id,
        JobFlagFilter::new(flags_to_skip, &[]),
        now,
    )
    .await
}

/// Claims the next registered job satisfying the flag filter.
///
/// Filtering happens before candidate locking and attempts are incremented.
/// A nonempty accepted set excludes jobs with absent or empty flags.
pub async fn get_job_with_filter(
    mut executor: impl DbExecutorArg,
    task_details: &TaskDetails,
    schema: impl Into<Schema>,
    worker_id: &str,
    filter: JobFlagFilter<'_>,
    now: Option<DateTime<Utc>>,
) -> Result<Option<Job>> {
    let schema = schema.into();
    let has_now = now.is_some();
    let now_param = has_now.then(|| 3 + filter.parameter_count());

    let sql =
        super::fetch_query_cache::fetch_query(&schema, filter.shape(), has_now, false, || {
            let flag_clause = filter.clause(3);
            let jobs = schema.private_table("jobs");
            let queue_clause = get_queue_clause(&schema);
            let update_queue_clause = get_update_queue_clause(&schema, 1, now_param);
            let now_clause = get_now_clause(now_param);

            formatdoc!(
                r#"
            with j as (
                select jobs.job_queue_id, jobs.priority, jobs.run_at, jobs.id
                    from {jobs} as jobs
                    where jobs.is_available = true
                    and run_at <= {now_clause}
                    and task_id = any($2::int[])
                    {queue_clause}
                    {flag_clause}
                    order by priority asc, run_at asc
                    limit 1
                    for update
                    skip locked
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
    ];
    filter.bind(&mut params);
    if let Some(ts) = now {
        params.push(DbValue::TimestampTz(ts));
    }

    let job = executor
        .fetch_optional(&sql, DbParams::from(params))
        .await?
        .map(|row| super::rows::db_job_from_row(&row))
        .transpose()?;
    Ok(job.map(|job| {
        let task_identifier = task_details.get_or_empty(job.id(), job.task_id());
        Job::from_db_job(job, task_identifier)
    }))
}
