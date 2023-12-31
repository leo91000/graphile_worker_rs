use chrono::prelude::*;
use getset::Getters;
use serde_json::Value;
use sqlx::{query_as, FromRow, PgExecutor};

use crate::errors::Result;

use super::task_identifiers::TaskDetails;

#[derive(FromRow, Getters, Debug)]
#[getset(get = "pub")]
#[allow(dead_code)]
pub struct Job {
    id: i64,
    /// FK to tasks
    job_queue_id: Option<i32>,
    /// The JSON payload of the job
    payload: serde_json::Value,
    /// Lower number means it should run sooner
    priority: i16,
    /// When it was due to run
    run_at: DateTime<Utc>,
    /// How many times it has been attempted
    attempts: i16,
    /// The limit for the number of times it should be attempted
    max_attempts: i16,
    /// If attempts > 0, why did it fail last ?
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// "job_key" - unique identifier for easy update from user code
    key: Option<String>,
    /// A count of the revision numbers
    revision: i32,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    flags: Option<Value>,
    /// Shorcut to tasks.identifer
    task_id: i32,
}

pub async fn get_job<'e>(
    executor: impl PgExecutor<'e>,
    task_details: &TaskDetails,
    escaped_schema: &str,
    worker_id: &str,
    flags_to_skip: &Vec<String>,
) -> Result<Option<Job>> {
    let flag_clause = get_flag_clause(flags_to_skip, 3);
    let queue_clause = get_queue_clause(escaped_schema);
    let update_queue_clause = get_update_queue_clause(escaped_schema, 1);

    let sql = format!(
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
                    limit 1
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

    let mut q = query_as(&sql).bind(worker_id).bind(task_details.task_ids());
    if !flags_to_skip.is_empty() {
        q = q.bind(flags_to_skip);
    }

    let job = q.fetch_optional(executor).await?;
    Ok(job)
}

fn get_flag_clause(flags_to_skip: &Vec<String>, param_ord: u8) -> String {
    if !flags_to_skip.is_empty() {
        return format!("and ((flags ?| ${param_ord}::text[]) is not true)");
    }
    "".into()
}

fn get_queue_clause(escaped_schema: &str) -> String {
    format!(
        r#"
            and (
                jobs.job_queue_id is null
                or 
                jobs.job_queue_id in (
                    select id 
                    from {escaped_schema}._private_job_queues as job_queues
                    where job_queues.is_available = true
                    for update
                    skip locked
                )
            )
        "#
    )
}

fn get_update_queue_clause(escaped_schema: &str, param_ord: u8) -> String {
    format!(
        r#",
            q as (
                update {escaped_schema}._private_job_queues as job_queues
                    set
                        locked_by = ${param_ord},
                        locked_at = now()
                from j
                where job_queues.id = j.job_queue_id
            )
        "#
    )
}
