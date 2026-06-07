use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use graphile_worker::JobSpec;

use super::super::super::error::ApiError;
use super::super::super::state::AppState;
use super::super::super::types::{
    db_job_output_from_job, job_key_mode_from_request, AddJobRequest, JobActionResponse,
};
use super::super::shared::ensure_write_allowed;

pub(crate) async fn add_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddJobRequest>,
) -> Result<Json<JobActionResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    validate_add_job_request(&request)?;

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

    let spec = add_job_spec(
        queue,
        run_at,
        max_attempts,
        key,
        job_key_mode,
        priority,
        flags,
    );
    let job = state
        .utils
        .add_raw_job(&identifier, payload, spec)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(JobActionResponse {
        message: format!("Added job {}", job.id()),
        jobs: vec![db_job_output_from_job(&job)],
    }))
}

fn validate_add_job_request(request: &AddJobRequest) -> Result<(), ApiError> {
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

    Ok(())
}

fn add_job_spec(
    queue: Option<String>,
    run_at: Option<chrono::DateTime<chrono::Utc>>,
    max_attempts: Option<i16>,
    key: Option<String>,
    job_key_mode: Option<super::super::super::types::JobKeyModeRequest>,
    priority: Option<i16>,
    flags: Option<Vec<String>>,
) -> graphile_worker::JobSpec {
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
        spec = spec.job_key_mode(job_key_mode_from_request(job_key_mode));
    }
    if let Some(priority) = priority {
        spec = spec.priority(priority);
    }
    if let Some(flags) = flags.filter(|flags| !flags.is_empty()) {
        spec = spec.flags(flags);
    }

    spec.build()
}
