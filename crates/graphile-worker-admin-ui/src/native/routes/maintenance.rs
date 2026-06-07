use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Json;
use graphile_worker::SweepStaleWorkersOptions;

use super::super::error::ApiError;
use super::super::state::AppState;
use super::super::types::{
    cleanup_task_from_name, CleanupTaskName, MaintenanceAction, MaintenanceRequest, MessageResponse,
};
use super::shared::ensure_write_allowed;

pub(crate) async fn maintenance(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MaintenanceRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    match request.action {
        MaintenanceAction::Migrate => {
            state.utils.migrate().await.map_err(ApiError::internal)?;
            Ok(Json(MessageResponse {
                message: "Migrations complete".to_string(),
            }))
        }
        MaintenanceAction::Cleanup => {
            let tasks = if request.cleanup_tasks.is_empty() {
                CleanupTaskName::all()
            } else {
                request.cleanup_tasks
            };
            let cleanup_tasks: Vec<_> = tasks.into_iter().map(cleanup_task_from_name).collect();
            state
                .utils
                .cleanup(&cleanup_tasks)
                .await
                .map_err(ApiError::internal)?;
            Ok(Json(MessageResponse {
                message: "Cleanup complete".to_string(),
            }))
        }
        MaintenanceAction::ForceUnlock => {
            if request.worker_ids.is_empty() {
                return Err(ApiError::bad_request("provide at least one worker id"));
            }
            let worker_ids: Vec<_> = request.worker_ids.iter().map(String::as_str).collect();
            state
                .utils
                .force_unlock_workers(&worker_ids)
                .await
                .map_err(ApiError::internal)?;
            Ok(Json(MessageResponse {
                message: format!("Unlocked {} worker id(s)", worker_ids.len()),
            }))
        }
        MaintenanceAction::SweepStaleWorkers => {
            let result = state
                .utils
                .sweep_stale_workers(SweepStaleWorkersOptions {
                    sweep_threshold: request.sweep_threshold_secs.map(Duration::from_secs),
                    recovery_delay: request.recovery_delay_secs.map(Duration::from_secs),
                    dry_run: request.dry_run,
                })
                .await
                .map_err(ApiError::internal)?;

            let message = if request.dry_run {
                if result.worker_ids.is_empty() {
                    "No stale workers found".to_string()
                } else {
                    format!(
                        "Would recover {} worker(s): {}",
                        result.worker_ids.len(),
                        result.worker_ids.join(", ")
                    )
                }
            } else if result.worker_ids.is_empty() {
                "No stale workers found".to_string()
            } else {
                format!(
                    "Recovered {} job(s) from {} worker(s): {}",
                    result.recovered_count,
                    result.worker_ids.len(),
                    result.worker_ids.join(", ")
                )
            };

            Ok(Json(MessageResponse { message }))
        }
    }
}
