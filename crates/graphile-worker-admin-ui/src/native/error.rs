use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use thiserror::Error;

use super::types::ErrorResponse;

#[derive(Debug, Error)]
pub enum AdminUiError {
    #[error("admin auth without credentials is only allowed on loopback addresses")]
    InsecureNoAuth,
    #[error("invalid header name `{0}`")]
    InvalidHeaderName(String),
    #[error("failed to bind admin UI listener: {0}")]
    Bind(#[from] std::io::Error),
    #[error("failed to serve admin UI: {0}")]
    Serve(std::io::Error),
}

pub(crate) fn json_error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
        .into_response()
}

pub(crate) type Result<T, E = ApiError> = core::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub(crate) fn internal(error: impl fmt::Display) -> Self {
        tracing::error!(error = %error, "admin UI request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_string(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::RowNotFound => Self::not_found("resource not found"),
            error => Self::internal(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        json_error_response(self.status, &self.message)
    }
}
