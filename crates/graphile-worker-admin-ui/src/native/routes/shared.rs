use super::super::error::ApiError;
use super::super::state::AppState;

pub(super) fn ensure_write_allowed(state: &AppState) -> Result<(), ApiError> {
    if state.read_only {
        return Err(ApiError::forbidden("admin UI is running in read-only mode"));
    }
    Ok(())
}
