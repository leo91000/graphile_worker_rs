use std::time::Duration;

use chrono::Utc;
use graphile_worker_database::{DbExecutorArg, DbParams, DbValue};
use indoc::formatdoc;

use crate::errors::Result;
use crate::recovery::ActiveWorkerRow;
use crate::sql::duration::duration_as_millis_i64;

/// Advisory lock namespace for coordinating stale worker sweeps.
const SWEEP_LOCK_CLASS_ID: i32 = 0x4757_5253; // "GWRS"
const SWEEP_LOCK_OBJECT_ID: i32 = 0x5357_4550; // "SWEP"

pub async fn worker_heartbeat(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_id: &str,
    metadata: Option<serde_json::Value>,
) -> Result<()> {
    let sql = formatdoc!(
        r#"
            SELECT * FROM {escaped_schema}.worker_heartbeat($1::text, $2::json);
        "#
    );

    executor
        .execute(
            &sql,
            vec![
                DbValue::Text(worker_id.to_string()),
                DbValue::JsonOpt(metadata),
            ]
            .into(),
        )
        .await?;

    Ok(())
}

pub async fn worker_deregister(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_id: &str,
) -> Result<()> {
    let sql = formatdoc!(
        r#"
            SELECT * FROM {escaped_schema}.worker_deregister($1::text);
        "#
    );

    executor
        .execute(&sql, vec![DbValue::Text(worker_id.to_string())].into())
        .await?;

    Ok(())
}

pub async fn list_stale_workers(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    stale_threshold: Duration,
) -> Result<Vec<String>> {
    let sql = formatdoc!(
        r#"
            SELECT worker_id
            FROM {escaped_schema}.list_stale_workers($1::bigint * interval '1 millisecond');
        "#
    );

    let rows = executor
        .fetch_all(
            &sql,
            DbParams::from(vec![DbValue::I64(duration_as_millis_i64(stale_threshold))]),
        )
        .await?;

    rows.iter()
        .map(|row| row.try_get::<String>("worker_id").map_err(Into::into))
        .collect()
}

pub async fn list_orphan_locked_workers(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    stale_threshold: Duration,
) -> Result<Vec<String>> {
    let sql = formatdoc!(
        r#"
            SELECT worker_id
            FROM {escaped_schema}.list_orphan_locked_workers($1::bigint * interval '1 millisecond');
        "#
    );

    let rows = executor
        .fetch_all(
            &sql,
            DbParams::from(vec![DbValue::I64(duration_as_millis_i64(stale_threshold))]),
        )
        .await?;

    rows.iter()
        .map(|row| row.try_get::<String>("worker_id").map_err(Into::into))
        .collect()
}

pub async fn list_active_workers(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    sweep_threshold: Duration,
) -> Result<Vec<ActiveWorkerRow>> {
    let sql = formatdoc!(
        r#"
            SELECT
                workers.id AS worker_id,
                workers.last_heartbeat_at,
                workers.started_at,
                workers.metadata
            FROM {escaped_schema}._private_workers AS workers
            ORDER BY workers.last_heartbeat_at DESC;
        "#
    );

    let rows = executor.fetch_all(&sql, DbParams::new()).await?;
    let now = Utc::now();

    rows.iter()
        .map(|row| {
            let last_heartbeat_at = row.try_get("last_heartbeat_at")?;
            let is_stale = now
                .signed_duration_since(last_heartbeat_at)
                .to_std()
                .ok()
                .is_some_and(|elapsed| elapsed >= sweep_threshold);
            Ok(ActiveWorkerRow {
                worker_id: row.try_get("worker_id")?,
                last_heartbeat_at,
                started_at: row.try_get("started_at")?,
                metadata: row.try_get("metadata")?,
                is_stale,
            })
        })
        .collect()
}

pub async fn worker_holds_resilient_locks(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_id: &str,
    resilient_flags: &[String],
) -> Result<bool> {
    if resilient_flags.is_empty() {
        return Ok(false);
    }

    let sql = formatdoc!(
        r#"
            SELECT EXISTS (
                SELECT 1
                FROM {escaped_schema}._private_jobs AS jobs
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

    row.try_get("has_resilient_locks").map_err(Into::into)
}

pub async fn get_worker_last_heartbeat(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_id: &str,
) -> Result<Option<chrono::DateTime<Utc>>> {
    let sql = formatdoc!(
        r#"
            SELECT workers.last_heartbeat_at
            FROM {escaped_schema}._private_workers AS workers
            WHERE workers.id = $1::text;
        "#
    );

    let row = executor
        .fetch_optional(
            &sql,
            DbParams::from(vec![DbValue::Text(worker_id.to_string())]),
        )
        .await?;

    row.map(|row| row.try_get("last_heartbeat_at"))
        .transpose()
        .map_err(Into::into)
}

pub async fn delete_stale_workers(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_ids: &[String],
) -> Result<()> {
    if worker_ids.is_empty() {
        return Ok(());
    }

    let sql = formatdoc!(
        r#"
            SELECT * FROM {escaped_schema}.delete_stale_workers($1::text[]);
        "#
    );

    executor
        .execute(&sql, vec![DbValue::TextArray(worker_ids.to_vec())].into())
        .await?;

    Ok(())
}

pub async fn try_acquire_sweep_lock(mut executor: impl DbExecutorArg) -> Result<bool> {
    let row = executor
        .fetch_optional(
            "SELECT pg_try_advisory_xact_lock($1::integer, $2::integer) AS acquired",
            DbParams::from(vec![
                DbValue::I32(SWEEP_LOCK_CLASS_ID),
                DbValue::I32(SWEEP_LOCK_OBJECT_ID),
            ]),
        )
        .await?;

    Ok(row
        .map(|row| row.try_get::<bool>("acquired"))
        .transpose()?
        .unwrap_or(false))
}
