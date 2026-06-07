use leptos::prelude::*;

#[component]
pub(super) fn Sidebar(schema: String, auth_mode: String, read_only: bool) -> impl IntoView {
    view! {
        <aside class="gw-sidebar">
            <div class="flex items-center gap-3">
                <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-600 text-white">
                    <span class="i-lucide-workflow h-5 w-5"></span>
                </div>
                <div>
                    <h1 class="text-base font-semibold">"Graphile Worker"</h1>
                    <p class="gw-muted text-xs">"Admin UI"</p>
                </div>
            </div>

            <nav class="mt-6 grid gap-1">
                <a class="gw-tab" href="#jobs" aria-selected="true">
                    <span class="i-lucide-list-checks h-4 w-4"></span>
                    "Jobs"
                </a>
                <a class="gw-tab" href="#queues">
                    <span class="i-lucide-git-branch h-4 w-4"></span>
                    "Queues"
                </a>
                <a class="gw-tab" href="#workers">
                    <span class="i-lucide-hard-drive h-4 w-4"></span>
                    "Workers"
                </a>
                <a class="gw-tab" href="#maintenance">
                    <span class="i-tabler-tool h-4 w-4"></span>
                    "Maintenance"
                </a>
            </nav>

            <div class="mt-auto grid gap-3 rounded-lg border p-3 text-xs" style="border-color: rgb(var(--border));">
                <div class="flex items-center justify-between">
                    <span class="gw-muted">"Schema"</span>
                    <span class="font-mono">{schema}</span>
                </div>
                <div class="flex items-center justify-between">
                    <span class="gw-muted">"Auth"</span>
                    <span id="auth-mode-label" class="gw-pill">{auth_mode}</span>
                </div>
                <div class="flex items-center justify-between">
                    <span class="gw-muted">"Writes"</span>
                    <span class="gw-pill">{if read_only { "read only" } else { "enabled" }}</span>
                </div>
            </div>
        </aside>
    }
}
