use graphile_worker_database::Schema;
use indoc::formatdoc;
use sqlx::PgPool;

use super::error::Result;
use crate::overview::{JobStats, QueueRow};
use crate::sql::{safe_query_as, AdminTable};

pub async fn get_stats(pool: &PgPool, schema: &Schema) -> Result<JobStats> {
    let jobs = AdminTable::Jobs.qualified(schema);
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
            from {jobs}
        "#
    );

    safe_query_as(sql.as_str())
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub async fn list_queues(pool: &PgPool, schema: &Schema) -> Result<Vec<QueueRow>> {
    let jobs = AdminTable::Jobs.qualified(schema);
    let job_queues = AdminTable::JobQueues.qualified(schema);
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
            from {job_queues} as job_queues
            left join {jobs} as jobs on jobs.job_queue_id = job_queues.id
            group by job_queues.id, job_queues.queue_name, job_queues.locked_at, job_queues.locked_by
            order by job_queues.queue_name asc
        "#
    );

    safe_query_as(sql.as_str())
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}
