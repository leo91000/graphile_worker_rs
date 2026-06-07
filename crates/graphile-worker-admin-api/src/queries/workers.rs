use graphile_worker_database::Schema;
use indoc::formatdoc;
use sqlx::PgPool;

use super::error::Result;
use crate::overview::LockedWorkerRow;
use crate::sql::{safe_query_as, AdminTable};

pub async fn list_locked_workers(pool: &PgPool, schema: &Schema) -> Result<Vec<LockedWorkerRow>> {
    let jobs = AdminTable::Jobs.qualified(schema);
    let job_queues = AdminTable::JobQueues.qualified(schema);
    let sql = formatdoc!(
        r#"
            select
                worker_id,
                sum(locked_jobs)::bigint as locked_jobs,
                sum(locked_queues)::bigint as locked_queues
            from (
                select locked_by as worker_id, count(*)::bigint as locked_jobs, 0::bigint as locked_queues
                from {jobs}
                where locked_by is not null
                group by locked_by
                union all
                select locked_by as worker_id, 0::bigint as locked_jobs, count(*)::bigint as locked_queues
                from {job_queues}
                where locked_by is not null
                group by locked_by
            ) as locks
            group by worker_id
            order by worker_id asc
        "#
    );

    safe_query_as(sql.as_str())
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}
