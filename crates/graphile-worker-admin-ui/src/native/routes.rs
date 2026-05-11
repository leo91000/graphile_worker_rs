use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::Json;
use chrono::Utc;
use graphile_worker::worker_utils::RescheduleJobOptions;
use graphile_worker::{DbJob, JobSpec};

use super::auth::CSRF_HEADER;
use super::error::{ApiError, Result};
use super::queries::{
    get_job, get_stats, list_jobs, list_locked_workers, list_queues, task_identifiers_by_id,
};
use super::state::AppState;
use super::types::{
    AddJobRequest, CleanupTaskName, DbJobOutput, JobAction, JobActionRequest, JobActionResponse,
    ListJobsParams, ListJobsResponse, ListedJob, MaintenanceAction, MaintenanceRequest,
    MessageResponse, OverviewResponse, RemoveJobByKeyRequest, SessionResponse,
};
use super::view::{render_admin_html, AdminUiRenderConfig};

pub(crate) async fn index(State(state): State<Arc<AppState>>) -> Html<String> {
    let auth = state.auth.summary();
    Html(render_admin_html(&AdminUiRenderConfig {
        csrf_token: state.csrf_token.clone(),
        schema: state.schema.clone(),
        read_only: state.read_only,
        auth,
    }))
}

pub(crate) async fn session(State(state): State<Arc<AppState>>) -> Json<SessionResponse> {
    Json(SessionResponse {
        schema: state.schema.clone(),
        read_only: state.read_only,
        csrf_header: CSRF_HEADER.to_string(),
        auth: state.auth.summary(),
    })
}

pub(crate) async fn overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OverviewResponse>, ApiError> {
    let stats = get_stats(&state.pool, &state.escaped_schema).await?;
    let queues = list_queues(&state.pool, &state.escaped_schema).await?;
    let workers = list_locked_workers(&state.pool, &state.escaped_schema).await?;
    Ok(Json(OverviewResponse {
        stats,
        queues,
        workers,
    }))
}

pub(crate) async fn jobs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListJobsParams>,
) -> Result<Json<ListJobsResponse>, ApiError> {
    let jobs = list_jobs(&state.pool, &state.escaped_schema, &params).await?;
    Ok(Json(ListJobsResponse { jobs }))
}

pub(crate) async fn job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ListedJob>, ApiError> {
    let job = get_job(&state.pool, &state.escaped_schema, id).await?;
    Ok(Json(job))
}

pub(crate) async fn add_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddJobRequest>,
) -> Result<Json<JobActionResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    if request.job_key_mode.is_some()
        && request
            .key
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    {
        return Err(ApiError::bad_request(
            "key is required when job_key_mode is set",
        ));
    }

    let AddJobRequest {
        identifier,
        payload,
        queue,
        run_at,
        max_attempts,
        key,
        job_key_mode,
        priority,
        flags,
    } = request;

    let mut spec = JobSpec::builder();
    if let Some(queue) = queue {
        spec = spec.queue_name(queue);
    }
    if let Some(run_at) = run_at {
        spec = spec.run_at(run_at);
    }
    if let Some(max_attempts) = max_attempts {
        spec = spec.max_attempts(max_attempts);
    }
    if let Some(key) = key {
        spec = spec.job_key(key);
    }
    if let Some(job_key_mode) = job_key_mode {
        spec = spec.job_key_mode(job_key_mode);
    }
    if let Some(priority) = priority {
        spec = spec.priority(priority);
    }
    if let Some(flags) = flags.filter(|flags| !flags.is_empty()) {
        spec = spec.flags(flags);
    }
    let spec = spec.build();

    let job = state
        .utils
        .add_raw_job(&identifier, payload, spec)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(JobActionResponse {
        message: format!("Added job {}", job.id()),
        jobs: vec![DbJobOutput::from_job(&job)],
    }))
}

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
        JobAction::Reschedule => {
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
            let cleanup_tasks: Vec<_> = tasks.into_iter().map(Into::into).collect();
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
    }
}

fn ensure_write_allowed(state: &AppState) -> Result<(), ApiError> {
    if state.read_only {
        return Err(ApiError::forbidden("admin UI is running in read-only mode"));
    }
    Ok(())
}

async fn action_response(
    state: &AppState,
    action: &str,
    jobs: &[DbJob],
) -> Result<Json<JobActionResponse>, ApiError> {
    let identifiers = task_identifiers_by_id(
        &state.pool,
        &state.escaped_schema,
        jobs.iter().map(|job| *job.task_id()),
    )
    .await?;

    Ok(Json(JobActionResponse {
        message: format!("{action} {} job(s)", jobs.len()),
        jobs: jobs
            .iter()
            .map(|job| DbJobOutput::from_db_job(job, identifiers.get(job.task_id()).cloned()))
            .collect(),
    }))
}
