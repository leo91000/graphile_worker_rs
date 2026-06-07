use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use subtle::ConstantTimeEq;

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
    left.ct_eq(right).into()
}
