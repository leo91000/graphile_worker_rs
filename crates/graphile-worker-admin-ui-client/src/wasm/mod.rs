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
use types::{AdminClientConfig, AuthMode};
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

    let config = match config_from_root(&root) {
        Ok(config) => config,
        Err(error) => {
            root.set_text_content(Some(&format!(
                "Failed to load Graphile Worker Admin UI: {error}"
            )));
            return;
        }
    };

    root.set_inner_html("");
    mount_to(root, move || view! { <AdminApp config=config /> }).forget();
}

fn config_from_root(root: &HtmlElement) -> Result<AdminClientConfig, String> {
    Ok(AdminClientConfig {
        auth_mode: AuthMode::parse(&required_data(root, "authMode")?)?,
        auth_header: root.dataset().get("authHeader").unwrap_or_default(),
        csrf: required_data(root, "csrf")?,
        csrf_header: required_data(root, "csrfHeader")?,
        read_only: parse_bool(&required_data(root, "readOnly")?)?,
        schema: required_data(root, "schema")?,
    })
}

fn required_data(root: &HtmlElement, key: &str) -> Result<String, String> {
    root.dataset()
        .get(key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing data-{key}"))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(format!("invalid read-only value `{value}`")),
    }
}
