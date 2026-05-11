mod api;
mod app;
mod browser;
mod components;
mod filters;
mod modals;
mod types;

use app::AdminApp;
use leptos::mount::mount_to;
use leptos::prelude::*;
use types::AdminClientConfig;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[wasm_bindgen(start)]
pub fn start() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.get_element_by_id("gw-admin") else {
        return;
    };
    let Ok(root) = root.dyn_into::<HtmlElement>() else {
        return;
    };

    let config = AdminClientConfig {
        auth_mode: data(&root, "authMode"),
        auth_header: data(&root, "authHeader"),
        csrf: data(&root, "csrf"),
        csrf_header: data(&root, "csrfHeader"),
        read_only: data(&root, "readOnly") == "true",
        schema: data(&root, "schema"),
    };

    root.set_inner_html("");
    mount_to(root, move || view! { <AdminApp config=config /> }).forget();
}

pub(super) fn data(root: &HtmlElement, key: &str) -> String {
    root.dataset().get(key).unwrap_or_default()
}
