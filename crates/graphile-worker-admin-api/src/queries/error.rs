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
