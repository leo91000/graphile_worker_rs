use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::response::IntoResponse;

pub const ADMIN_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/admin.css"));
pub const ADMIN_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/admin.js"));
pub const ADMIN_WASM_BINDGEN_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/admin_ui.js"));
pub const ADMIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/admin_ui_bg.wasm"));
pub const ADMIN_FAVICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="12" fill="#0e7490"/><path d="M17 22h30M17 32h30M17 42h30" stroke="#fff" stroke-width="6" stroke-linecap="round"/></svg>"##;

pub(crate) async fn css() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        ADMIN_CSS,
    )
}

pub(crate) async fn js() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        ADMIN_JS,
    )
}

pub(crate) async fn wasm_bindgen_js() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        ADMIN_WASM_BINDGEN_JS,
    )
}

pub(crate) async fn wasm() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("application/wasm"))],
        ADMIN_WASM,
    )
}

pub(crate) async fn favicon() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
        ADMIN_FAVICON,
    )
}
