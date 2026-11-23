use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, DbParams, Schema};
use indoc::formatdoc;
use time::OffsetDateTime;

use crate::errors::Result;

use crate::rows::get_required;
use crate::schema_names::PrivateTable;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    #[serde(serialize_with = "graphile_worker_job::serialize_utc_timestamp")]
    pub last_heartbeat_at: OffsetDateTime,
    #[serde(serialize_with = "graphile_worker_job::serialize_utc_timestamp")]
    pub started_at: OffsetDateTime,
    pub metadata: Option<serde_json::Value>,
    pub is_stale: bool,
}

pub async fn list_active_workers(
    mut executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    sweep_threshold: Duration,
) -> Result<Vec<ActiveWorkerRow>> {
    let schema = schema.into();
    let workers = PrivateTable::Workers.qualified(&schema);
    let sql = formatdoc!(
        r#"
            SELECT
                workers.id AS worker_id,
                workers.last_heartbeat_at,
                workers.started_at,
                workers.metadata
            FROM {workers} AS workers
            ORDER BY workers.last_heartbeat_at DESC;
        "#
    );

    let rows = executor.fetch_all(&sql, DbParams::new()).await?;
    let now = OffsetDateTime::now_utc();

    rows.iter()
        .map(|row| {
            let last_heartbeat_at = get_required(row, "last_heartbeat_at")?;
            let is_stale = Duration::try_from(now - last_heartbeat_at)
                .ok()
                .is_some_and(|elapsed| elapsed >= sweep_threshold);
            Ok(ActiveWorkerRow {
                worker_id: get_required(row, "worker_id")?,
                last_heartbeat_at,
                started_at: get_required(row, "started_at")?,
                metadata: get_required(row, "metadata")?,
                is_stale,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_worker_round_trips_signed_years_to_json() {
        let timestamp = time::macros::datetime!(+10000-01-01 00:00 UTC);
        let row = ActiveWorkerRow {
            worker_id: "worker".into(),
            last_heartbeat_at: timestamp,
            started_at: timestamp,
            metadata: None,
            is_stale: false,
        };
        let json = serde_json::to_value(row).unwrap();
        assert_eq!(json["last_heartbeat_at"], "+10000-01-01T00:00:00Z");
        assert_eq!(json["started_at"], "+10000-01-01T00:00:00Z");
    }
}
