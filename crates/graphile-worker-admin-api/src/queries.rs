use std::collections::{BTreeSet, HashMap};

use indoc::formatdoc;
use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::{JobState, JobStats, ListJobsParams, ListedJob, LockedWorkerRow, QueueRow};

pub type Result<T> = core::result::Result<T, AdminQueryError>;

#[derive(Debug, thiserror::Error)]
pub enum AdminQueryError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

impl AdminQueryError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListJobsQueryOptions {
    pub max_limit: Option<i64>,
}

pub async fn list_jobs(
    pool: &PgPool,
    escaped_schema: &str,
    args: &ListJobsParams,
    options: ListJobsQueryOptions,
) -> Result<Vec<ListedJob>> {
    if args.limit < 0 {
        return Err(AdminQueryError::bad_request(
            "limit must be greater than or equal to 0",
        ));
    }
    if args.offset < 0 {
        return Err(AdminQueryError::bad_request(
            "offset must be greater than or equal to 0",
        ));
    }

    let limit = options
        .max_limit
        .map_or(args.limit, |max_limit| args.limit.min(max_limit));
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
                jobs.priority,
                jobs.run_at,
                jobs.attempts,
                jobs.max_attempts,
                jobs.last_error,
                jobs.created_at,
                jobs.updated_at,
                jobs.key,
                jobs.locked_at,
                jobs.locked_by,
                jobs.revision,
                jobs.flags,
                jobs.is_available
            from {escaped_schema}._private_jobs as jobs
            inner join {escaped_schema}._private_tasks as tasks on tasks.id = jobs.task_id
            left join {escaped_schema}._private_job_queues as job_queues on job_queues.id = jobs.job_queue_id
            where true
        "#
    ));

    apply_job_filters(&mut query, args);
    query.push(" order by jobs.id asc limit ");
    query.push_bind(limit);
    query.push(" offset ");
    query.push_bind(args.offset);

    query
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

pub async fn get_job(pool: &PgPool, escaped_schema: &str, id: i64) -> Result<ListedJob> {
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
                jobs.priority,
                jobs.run_at,
                jobs.attempts,
                jobs.max_attempts,
                jobs.last_error,
                jobs.created_at,
                jobs.updated_at,
                jobs.key,
                jobs.locked_at,
                jobs.locked_by,
                jobs.revision,
                jobs.flags,
                jobs.is_available
            from {escaped_schema}._private_jobs as jobs
            inner join {escaped_schema}._private_tasks as tasks on tasks.id = jobs.task_id
            left join {escaped_schema}._private_job_queues as job_queues on job_queues.id = jobs.job_queue_id
            where jobs.id =
        "#
    ));
    query.push_bind(id);

    query
        .build_query_as()
        .fetch_one(pool)
        .await
        .map_err(|error| match error {
            sqlx::Error::RowNotFound => AdminQueryError::not_found(format!("job {id} not found")),
            error => error.into(),
        })
}

pub async fn task_identifiers_by_id(
    pool: &PgPool,
    escaped_schema: &str,
    task_ids: impl IntoIterator<Item = i32>,
) -> Result<HashMap<i32, String>> {
    let task_ids = task_ids.into_iter().collect::<BTreeSet<_>>();
    if task_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select id, identifier
            from {escaped_schema}._private_tasks
            where id in (
        "#
    ));
    let mut separated = query.separated(", ");
    for task_id in task_ids {
        separated.push_bind(task_id);
    }
    separated.push_unseparated(")");

    Ok(query
        .build_query_as::<(i32, String)>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect())
}

pub fn apply_job_filters(query: &mut QueryBuilder<Postgres>, args: &ListJobsParams) {
    if let Some(identifier) = args.identifier.as_ref().filter(|value| !value.is_empty()) {
        query.push(" and tasks.identifier = ");
        query.push_bind(identifier);
    }

    if let Some(queue) = args.queue.as_ref().filter(|value| !value.is_empty()) {
        query.push(" and job_queues.queue_name = ");
        query.push_bind(queue);
    }

    if let Some(search) = args
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let pattern = format!("%{search}%");
        query.push(" and (jobs.id::text ilike ");
        query.push_bind(pattern.clone());
        query.push(" or tasks.identifier ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(job_queues.queue_name, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(jobs.key, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(jobs.locked_by, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(jobs.last_error, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or jobs.payload::text ilike ");
        query.push_bind(pattern);
        query.push(")");
    }

    match args.state {
        JobState::All => {}
        JobState::Ready => {
            query.push(
                " and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at <= now()",
            );
        }
        JobState::Scheduled => {
            query.push(
                " and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at > now()",
            );
        }
        JobState::Locked => {
            query.push(" and jobs.locked_at is not null");
        }
        JobState::Failed => {
            query.push(" and jobs.locked_at is null and jobs.attempts >= jobs.max_attempts");
        }
    }
}

pub async fn get_stats(pool: &PgPool, escaped_schema: &str) -> Result<JobStats> {
    let sql = formatdoc!(
        r#"
            select
                count(*)::bigint as total,
                count(*) filter (
                    where locked_at is null
                    and attempts < max_attempts
                    and run_at <= now()
                )::bigint as ready,
                count(*) filter (
                    where locked_at is null
                    and attempts < max_attempts
                    and run_at > now()
                )::bigint as scheduled,
                count(*) filter (where locked_at is not null)::bigint as locked,
                count(*) filter (
                    where locked_at is null
                    and attempts >= max_attempts
                )::bigint as failed
            from {escaped_schema}._private_jobs
        "#
    );

    sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub async fn list_queues(pool: &PgPool, escaped_schema: &str) -> Result<Vec<QueueRow>> {
    let sql = formatdoc!(
        r#"
            select
                job_queues.id,
                job_queues.queue_name,
                job_queues.locked_at,
                job_queues.locked_by,
                count(jobs.*)::bigint as job_count,
                count(jobs.*) filter (
                    where jobs.locked_at is null
                    and jobs.attempts < jobs.max_attempts
                    and jobs.run_at <= now()
                )::bigint as ready_count
            from {escaped_schema}._private_job_queues as job_queues
            left join {escaped_schema}._private_jobs as jobs on jobs.job_queue_id = job_queues.id
            group by job_queues.id, job_queues.queue_name, job_queues.locked_at, job_queues.locked_by
            order by job_queues.queue_name asc
        "#
    );

    sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

pub async fn list_locked_workers(
    pool: &PgPool,
    escaped_schema: &str,
) -> Result<Vec<LockedWorkerRow>> {
    let sql = formatdoc!(
        r#"
            select
                worker_id,
                sum(locked_jobs)::bigint as locked_jobs,
                sum(locked_queues)::bigint as locked_queues
            from (
                select locked_by as worker_id, count(*)::bigint as locked_jobs, 0::bigint as locked_queues
                from {escaped_schema}._private_jobs
                where locked_by is not null
                group by locked_by
                union all
                select locked_by as worker_id, 0::bigint as locked_jobs, count(*)::bigint as locked_queues
                from {escaped_schema}._private_job_queues
                where locked_by is not null
                group by locked_by
            ) as locks
            group by worker_id
            order by worker_id asc
        "#
    );

    sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}
