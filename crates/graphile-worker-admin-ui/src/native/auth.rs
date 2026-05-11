use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, HeaderName};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::Rng;
use serde::Serialize;

use super::error::AdminUiError;

pub(crate) const CSRF_HEADER: &str = "x-graphile-worker-admin-csrf";

#[derive(Clone, Debug, Serialize)]
pub enum PublicAuthMode {
    Basic,
    Bearer,
    Header,
    None,
}

impl PublicAuthMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Bearer => "bearer",
            Self::Header => "header",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Debug)]
pub enum AdminAuthConfig {
    Basic {
        username: String,
        password: String,
        generated_password: bool,
    },
    Bearer {
        token: String,
        generated_token: bool,
    },
    Header {
        header_name: HeaderName,
        token: String,
        generated_token: bool,
    },
    None,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdminAuthSummary {
    pub mode: PublicAuthMode,
    pub username: Option<String>,
    pub header_name: Option<String>,
    pub generated_secret: bool,
}

impl AdminAuthConfig {
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
            generated_password: false,
        }
    }

    pub fn basic_with_random_password(username: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: generate_secret(),
            generated_password: true,
        }
    }

    pub fn bearer(token: impl Into<String>, generated_token: bool) -> Self {
        Self::Bearer {
            token: token.into(),
            generated_token,
        }
    }

    pub fn header(
        header_name: impl AsRef<str>,
        token: impl Into<String>,
        generated_token: bool,
    ) -> Result<Self, AdminUiError> {
        let header_name = HeaderName::from_bytes(header_name.as_ref().as_bytes())
            .map_err(|_| AdminUiError::InvalidHeaderName(header_name.as_ref().to_string()))?;
        Ok(Self::Header {
            header_name,
            token: token.into(),
            generated_token,
        })
    }

    pub fn summary(&self) -> AdminAuthSummary {
        match self {
            Self::Basic {
                username,
                generated_password,
                ..
            } => AdminAuthSummary {
                mode: PublicAuthMode::Basic,
                username: Some(username.clone()),
                header_name: None,
                generated_secret: *generated_password,
            },
            Self::Bearer {
                generated_token, ..
            } => AdminAuthSummary {
                mode: PublicAuthMode::Bearer,
                username: None,
                header_name: None,
                generated_secret: *generated_token,
            },
            Self::Header {
                header_name,
                generated_token,
                ..
            } => AdminAuthSummary {
                mode: PublicAuthMode::Header,
                username: None,
                header_name: Some(header_name.as_str().to_string()),
                generated_secret: *generated_token,
            },
            Self::None => AdminAuthSummary {
                mode: PublicAuthMode::None,
                username: None,
                header_name: None,
                generated_secret: false,
            },
        }
    }

    pub fn secret_for_display(&self) -> Option<&str> {
        match self {
            Self::Basic { password, .. } => Some(password),
            Self::Bearer { token, .. } | Self::Header { token, .. } => Some(token),
            Self::None => None,
        }
    }

    pub(crate) fn is_authorized(&self, headers: &HeaderMap) -> bool {
        match self {
            Self::None => true,
            Self::Basic {
                username, password, ..
            } => authorize_basic(headers, username, password),
            Self::Bearer { token, .. } => authorize_bearer(headers, token),
            Self::Header {
                header_name, token, ..
            } => headers
                .get(header_name)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| constant_time_eq(value.as_bytes(), token.as_bytes())),
        }
    }
}

pub fn generate_secret() -> String {
    let mut bytes = [0_u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub(crate) fn authorize_basic(
    headers: &HeaderMap,
    expected_user: &str,
    expected_password: &str,
) -> bool {
    let Some(header) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(encoded) = header.strip_prefix("Basic ") else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };
    let Ok(credentials) = String::from_utf8(decoded) else {
        return false;
    };
    let Some((user, password)) = credentials.split_once(':') else {
        return false;
    };

    constant_time_eq(user.as_bytes(), expected_user.as_bytes())
        & constant_time_eq(password.as_bytes(), expected_password.as_bytes())
}

pub(crate) fn authorize_bearer(headers: &HeaderMap, expected_token: &str) -> bool {
    let Some(header) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(token) = header.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_eq(token.as_bytes(), expected_token.as_bytes())
}

pub(crate) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        diff |= usize::from(left_byte ^ right_byte);
    }
    diff == 0
}
