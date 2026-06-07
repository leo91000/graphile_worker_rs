use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use graphile_worker::recovery::WorkerRecoveryConfig;
use graphile_worker_admin_api::queries::overview::{get_stats, list_queues};
use graphile_worker_admin_api::queries::workers::list_locked_workers;

use super::super::error::ApiError;
use super::super::state::AppState;
use super::super::types::{ActiveWorkerRow, OverviewResponse};

pub(crate) async fn overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OverviewResponse>, ApiError> {
    let stats = get_stats(&state.pool, &state.schema).await?;
    let queues = list_queues(&state.pool, &state.schema).await?;
    let workers = list_locked_workers(&state.pool, &state.schema).await?;
    let recovery_defaults = WorkerRecoveryConfig::default();
    let active_workers = state
        .utils
        .list_active_workers(recovery_defaults.sweep_threshold)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|worker| ActiveWorkerRow {
            worker_id: worker.worker_id,
            last_heartbeat_at: worker.last_heartbeat_at,
            started_at: worker.started_at,
            metadata: worker.metadata,
            is_stale: worker.is_stale,
        })
        .collect();
    Ok(Json(OverviewResponse {
        stats,
        queues,
        workers,
        active_workers,
    }))
}
