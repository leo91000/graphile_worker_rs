use leptos::prelude::*;

use super::jobs::JobsPanel;
use super::panels::QueuesWorkersPanel;

#[component]
pub(super) fn AdminContent() -> impl IntoView {
    view! {
        <div class="gw-scroll">
            <TokenLogin />
            <Overview />
            <JobsPanel />
            <QueuesWorkersPanel />
            <Maintenance />
        </div>
    }
}

#[component]
fn TokenLogin() -> impl IntoView {
    view! {
        <section id="token-login" class="gw-panel mb-4 hidden p-4">
            <div class="flex flex-wrap items-end gap-3">
                <div class="min-w-72 flex-1">
                    <label class="mb-1 block text-sm font-medium" for="auth-token">"API token"</label>
                    <input id="auth-token" class="gw-input w-full" type="password" autocomplete="current-password" />
                </div>
                <button id="save-token-btn" class="gw-btn gw-btn-primary" type="button">
                    <span class="i-lucide-key-round h-4 w-4"></span>
                    "Use token"
                </button>
                <button id="clear-token-btn" class="gw-btn" type="button">"Clear"</button>
            </div>
        </section>
    }
}

#[component]
fn Overview() -> impl IntoView {
    view! {
        <section id="overview" class="grid gap-3 md:grid-cols-5">
            <StatCard id="stat-total" label="Total" icon="i-lucide-database h-4 w-4 gw-muted" />
            <StatCard id="stat-ready" label="Ready" icon="i-lucide-play h-4 w-4 text-emerald-500" />
            <StatCard id="stat-scheduled" label="Scheduled" icon="i-lucide-clock h-4 w-4 text-amber-500" />
            <StatCard id="stat-locked" label="Locked" icon="i-lucide-lock h-4 w-4 text-cyan-500" />
            <StatCard id="stat-failed" label="Failed" icon="i-lucide-circle-alert h-4 w-4 text-rose-500" />
        </section>
    }
}

#[component]
fn StatCard(id: &'static str, label: &'static str, icon: &'static str) -> impl IntoView {
    view! {
        <div class="gw-panel p-4">
            <div class="flex items-center justify-between">
                <span class="gw-muted text-sm">{label}</span>
                <span class=icon></span>
            </div>
            <strong id=id class="mt-2 block text-2xl">"0"</strong>
        </div>
    }
}

#[component]
fn Maintenance() -> impl IntoView {
    view! {
        <section id="maintenance" class="gw-panel mt-4 p-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <h3 class="font-semibold">"Maintenance"</h3>
                    <p class="gw-muted text-sm">"Run migrations, cleanup orphaned queue metadata, and recover abandoned locks."</p>
                </div>
                <div class="flex flex-wrap gap-2">
                    <button id="migrate-btn" class="gw-btn" type="button">
                        <span class="i-lucide-database-zap h-4 w-4"></span>
                        "Migrate"
                    </button>
                    <button id="cleanup-btn" class="gw-btn" type="button">
                        <span class="i-lucide-sparkles h-4 w-4"></span>
                        "Cleanup"
                    </button>
                </div>
            </div>
        </section>
    }
}
