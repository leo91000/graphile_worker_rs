use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, REFERRER_POLICY, WWW_AUTHENTICATE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use super::auth::{constant_time_eq, AdminAuthConfig, CSRF_HEADER};
use super::error::json_error_response;
use super::state::AppState;

pub(crate) async fn require_page_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if state.auth.is_authorized(request.headers()) {
        next.run(request).await
    } else {
        unauthorized_response(&state.auth)
    }
}

pub(crate) async fn require_api_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !state.auth.is_authorized(request.headers()) {
        return unauthorized_response(&state.auth);
    }

    if is_write_method(request.method()) && !csrf_matches(request.headers(), &state.csrf_token) {
        return json_error_response(StatusCode::FORBIDDEN, "missing or invalid admin CSRF token");
    }

    next.run(request).await
}

pub(crate) async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let cache_policy = if is_static_asset(request.uri().path()) {
        "public, max-age=3600"
    } else {
        "no-store"
    };
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_str(cache_policy).expect("static cache policy is valid"),
    );
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    response
}

pub(crate) fn unauthorized_response(auth: &AdminAuthConfig) -> Response {
    let mut response = json_error_response(StatusCode::UNAUTHORIZED, "unauthorized");
    if matches!(auth, AdminAuthConfig::Basic { .. }) {
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Graphile Worker Admin\", charset=\"UTF-8\""),
        );
    }
    response
}

fn csrf_matches(headers: &HeaderMap, expected_token: &str) -> bool {
    headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| constant_time_eq(value.as_bytes(), expected_token.as_bytes()))
}

fn is_write_method(method: &Method) -> bool {
    !matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

fn is_static_asset(path: &str) -> bool {
    path.starts_with("/assets/") || path == "/favicon.ico"
}
