#[cfg(target_arch = "wasm32")]
compile_error!(
    "graphile_worker_admin_ui is a native Axum server crate; use graphile_worker_admin_ui_client for the WASM client."
);

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
