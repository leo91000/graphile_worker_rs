mod content;
mod html;
mod jobs;
mod modal;
mod panels;
mod shell;
mod sidebar;
mod topbar;

use leptos::prelude::*;

use self::shell::AdminShell;
use super::auth::AdminAuthSummary;

#[derive(Debug)]
pub struct AdminUiRenderConfig {
    pub csrf_token: String,
    pub schema: String,
    pub read_only: bool,
    pub auth: AdminAuthSummary,
}

pub fn render_admin_html(config: &AdminUiRenderConfig) -> String {
    let app = view! {
        <AdminShell
            auth_mode=config.auth.mode.as_str().to_string()
            header_name=config.auth.header_name.clone().unwrap_or_default()
            read_only_text=config.read_only.to_string()
            csrf_token=config.csrf_token.clone()
            schema=config.schema.clone()
            read_only=config.read_only
        />
    };

    html::render_document(app.to_html())
}
