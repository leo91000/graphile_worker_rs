use std::time::Duration;

use chrono::Utc;
use graphile_worker_database::{DbExecutorArg, DbParams, DbValue, Schema};
use indoc::formatdoc;

use crate::duration::duration_as_millis_i64;
use crate::errors::Result;
use crate::rows::{collect_column, get_required};
use crate::schema_names::{PrivateTable, WorkerFunction};

pub async fn list_stale_workers(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    stale_threshold: Duration,
) -> Result<Vec<String>> {
    let list_stale_workers = WorkerFunction::ListStaleWorkers.qualified(schema);
    list_workers_from_threshold_function(
        &mut executor,
        list_stale_workers.to_string(),
        stale_threshold,
    )
    .await
}

pub async fn list_orphan_locked_workers(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    stale_threshold: Duration,
) -> Result<Vec<String>> {
    let list_orphan_locked_workers = WorkerFunction::ListOrphanLockedWorkers.qualified(schema);
    list_workers_from_threshold_function(
        &mut executor,
        list_orphan_locked_workers.to_string(),
        stale_threshold,
    )
    .await
}

async fn list_workers_from_threshold_function(
    executor: &mut impl DbExecutorArg,
    function: String,
    threshold: Duration,
) -> Result<Vec<String>> {
    let sql = formatdoc!(
        r#"
            SELECT worker_id
            FROM {function}($1::bigint * interval '1 millisecond');
        "#
    );

    let rows = executor
        .fetch_all(
            &sql,
            DbParams::from(vec![DbValue::I64(duration_as_millis_i64(threshold))]),
        )
        .await?;

    collect_column(&rows, "worker_id")
}

pub async fn worker_holds_resilient_locks(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    worker_id: &str,
    resilient_flags: &[String],
) -> Result<bool> {
    if resilient_flags.is_empty() {
        return Ok(false);
    }

    let jobs = PrivateTable::Jobs.qualified(schema);
    let sql = formatdoc!(
        r#"
            SELECT EXISTS (
                SELECT 1
                FROM {jobs} AS jobs
                WHERE jobs.locked_by = $1::text
                AND jobs.flags ?| $2::text[]
            ) AS has_resilient_locks;
        "#
    );

    let row = executor
        .fetch_one(
            &sql,
            DbParams::from(vec![
                DbValue::Text(worker_id.to_string()),
                DbValue::TextArray(resilient_flags.to_vec()),
            ]),
        )
        .await?;

    get_required(&row, "has_resilient_locks")
}

pub async fn get_worker_last_heartbeat(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    worker_id: &str,
) -> Result<Option<chrono::DateTime<Utc>>> {
    let workers = PrivateTable::Workers.qualified(schema);
    let sql = formatdoc!(
        r#"
            SELECT workers.last_heartbeat_at
            FROM {workers} AS workers
            WHERE workers.id = $1::text;
        "#
    );

    let row = executor
        .fetch_optional(
            &sql,
            DbParams::from(vec![DbValue::Text(worker_id.to_string())]),
        )
        .await?;

    row.map(|row| get_required(&row, "last_heartbeat_at"))
        .transpose()
}

pub async fn delete_stale_workers(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    worker_ids: &[String],
) -> Result<()> {
    if worker_ids.is_empty() {
        return Ok(());
    }

    let delete_stale_workers = WorkerFunction::DeleteStaleWorkers.qualified(schema);
    let sql = formatdoc!(
        r#"
            SELECT * FROM {delete_stale_workers}($1::text[]);
        "#
    );

    executor
        .execute(&sql, vec![DbValue::TextArray(worker_ids.to_vec())].into())
        .await?;

    Ok(())
}
