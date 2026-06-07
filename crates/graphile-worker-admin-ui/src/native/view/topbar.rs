use leptos::prelude::*;

#[component]
pub(super) fn Topbar() -> impl IntoView {
    view! {
        <header class="gw-topbar">
            <div class="min-w-0">
                <p class="gw-muted text-xs">"PostgreSQL-backed queue control plane"</p>
                <h2 class="truncate text-lg font-semibold">"Jobs, queues, and workers"</h2>
            </div>
            <div class="flex flex-wrap items-center justify-end gap-2">
                <select id="theme-select" class="gw-input w-32" aria-label="Theme">
                    <option value="system">"System"</option>
                    <option value="light">"Light"</option>
                    <option value="dark">"Dark"</option>
                </select>
                <select id="accent-select" class="gw-input w-32" aria-label="Accent">
                    <option value="cyan">"Cyan"</option>
                    <option value="emerald">"Emerald"</option>
                    <option value="violet">"Violet"</option>
                    <option value="amber">"Amber"</option>
                </select>
                <button id="density-toggle" class="gw-btn" type="button" title="Toggle density">
                    <span class="i-lucide-align-justify h-4 w-4"></span>
                </button>
                <label class="gw-btn cursor-pointer">
                    <input id="auto-refresh" class="h-4 w-4" type="checkbox" />
                    <span class="text-sm">"Auto"</span>
                </label>
                <button id="refresh-btn" class="gw-btn gw-btn-primary" type="button">
                    <span class="i-lucide-refresh-cw h-4 w-4"></span>
                    "Refresh"
                </button>
            </div>
        </header>
    }
}
