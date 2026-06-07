use leptos::prelude::*;

use super::super::auth::CSRF_HEADER;
use super::content::AdminContent;
use super::modal::AdminModal;
use super::sidebar::Sidebar;
use super::topbar::Topbar;

#[component]
pub(super) fn AdminShell(
    auth_mode: String,
    header_name: String,
    read_only_text: String,
    csrf_token: String,
    schema: String,
    read_only: bool,
) -> impl IntoView {
    let shell_auth_mode = auth_mode.clone();
    let sidebar_auth_mode = auth_mode;
    let sidebar_schema = schema.clone();

    view! {
        <div
            id="gw-admin"
            class="gw-shell"
            data-auth-mode=shell_auth_mode
            data-auth-header=header_name
            data-read-only=read_only_text
            data-csrf=csrf_token
            data-csrf-header=CSRF_HEADER
            data-schema=schema
        >
            <Sidebar schema=sidebar_schema auth_mode=sidebar_auth_mode read_only=read_only />
            <main class="gw-main">
                <Topbar />
                <AdminContent />
            </main>
            <AdminModal />
            <div id="toast" class="gw-toast"></div>
        </div>
    }
}
