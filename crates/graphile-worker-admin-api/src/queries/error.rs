pub type Result<T> = core::result::Result<T, AdminQueryError>;

#[derive(Debug, thiserror::Error)]
pub enum AdminQueryError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

impl AdminQueryError {
    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_query_errors_preserve_messages() {
        let bad_request = AdminQueryError::bad_request("invalid limit");
        let not_found = AdminQueryError::not_found("job not found");

        assert!(matches!(bad_request, AdminQueryError::BadRequest(_)));
        assert_eq!(bad_request.to_string(), "invalid limit");
        assert!(matches!(not_found, AdminQueryError::NotFound(_)));
        assert_eq!(not_found.to_string(), "job not found");
    }
}
