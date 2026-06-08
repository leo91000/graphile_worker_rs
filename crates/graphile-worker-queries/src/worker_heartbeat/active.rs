use std::time::Duration;

use chrono::{DateTime, Utc};
use graphile_worker_database::{DbExecutorArg, DbParams, Schema};
use indoc::formatdoc;

use crate::errors::Result;

use crate::rows::get_required;
use crate::schema_names::PrivateTable;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    pub last_heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub is_stale: bool,
}

pub async fn list_active_workers(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    sweep_threshold: Duration,
) -> Result<Vec<ActiveWorkerRow>> {
    let workers = PrivateTable::Workers.qualified(schema);
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
    let now = Utc::now();

    rows.iter()
        .map(|row| {
            let last_heartbeat_at = get_required(row, "last_heartbeat_at")?;
            let is_stale = now
                .signed_duration_since(last_heartbeat_at)
                .to_std()
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
