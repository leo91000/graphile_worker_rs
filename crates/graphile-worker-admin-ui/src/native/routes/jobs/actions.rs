use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use chrono::Utc;
use graphile_worker::worker_utils::types::RescheduleJobOptions;
use graphile_worker::DbJob;
use graphile_worker_admin_api::queries::jobs::tasks::task_identifiers_by_id;

use super::super::super::error::ApiError;
use super::super::super::state::AppState;
use super::super::super::types::{
    db_job_output_from_db_job, JobAction, JobActionRequest, JobActionResponse, MessageResponse,
    RemoveJobByKeyRequest,
};
use super::super::shared::ensure_write_allowed;

pub(crate) async fn job_action(
    State(state): State<Arc<AppState>>,
    Json(request): Json<JobActionRequest>,
) -> Result<Json<JobActionResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    if request.ids.is_empty() {
        return Err(ApiError::bad_request("select at least one job"));
    }

    match request.action {
        JobAction::Complete => {
            let jobs = state
                .utils
                .complete_jobs(&request.ids)
                .await
                .map_err(ApiError::internal)?;
            action_response(&state, "Completed", &jobs).await
        }
        JobAction::Fail => {
            let reason = request
                .reason
                .as_deref()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or("Marked failed from Graphile Worker admin UI");
            let jobs = state
                .utils
                .permanently_fail_jobs(&request.ids, reason)
                .await
                .map_err(ApiError::internal)?;
            action_response(&state, "Failed", &jobs).await
        }
        JobAction::RunNow => {
            let jobs = state
                .utils
                .reschedule_jobs(
                    &request.ids,
                    RescheduleJobOptions {
                        run_at: Some(Utc::now()),
                        priority: request.priority,
                        attempts: request.attempts,
                        max_attempts: request.max_attempts,
                    },
                )
                .await
                .map_err(ApiError::internal)?;
            action_response(&state, "Run now", &jobs).await
        }
        JobAction::Reschedule => reschedule_jobs(state, request).await,
    }
}

pub(crate) async fn remove_job_by_key(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RemoveJobByKeyRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    if request.key.trim().is_empty() {
        return Err(ApiError::bad_request("key is required"));
    }

    state
        .utils
        .remove_job(&request.key)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(MessageResponse {
        message: format!("Removed job with key `{}`", request.key),
    }))
}

async fn reschedule_jobs(
    state: Arc<AppState>,
    request: JobActionRequest,
) -> Result<Json<JobActionResponse>, ApiError> {
    if request.run_at.is_none()
        && request.priority.is_none()
        && request.attempts.is_none()
        && request.max_attempts.is_none()
    {
        return Err(ApiError::bad_request(
            "provide run_at, priority, attempts, or max_attempts",
        ));
    }

    let jobs = state
        .utils
        .reschedule_jobs(
            &request.ids,
            RescheduleJobOptions {
                run_at: request.run_at,
                priority: request.priority,
                attempts: request.attempts,
                max_attempts: request.max_attempts,
            },
        )
        .await
        .map_err(ApiError::internal)?;
    action_response(&state, "Rescheduled", &jobs).await
}

async fn action_response(
    state: &AppState,
    action: &str,
    jobs: &[DbJob],
) -> Result<Json<JobActionResponse>, ApiError> {
    let identifiers = task_identifiers_by_id(
        &state.pool,
        &state.schema,
        jobs.iter().map(|job| *job.task_id()),
    )
    .await?;

    Ok(Json(JobActionResponse {
        message: format!("{action} {} job(s)", jobs.len()),
        jobs: jobs
            .iter()
            .map(|job| db_job_output_from_db_job(job, identifiers.get(job.task_id()).cloned()))
            .collect(),
    }))
}
