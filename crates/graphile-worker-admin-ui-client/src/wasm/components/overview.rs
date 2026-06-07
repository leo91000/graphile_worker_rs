use leptos::prelude::*;

use super::super::types::JobStats;

#[component]
pub(in crate::wasm) fn Overview(
    stats: impl Fn() -> JobStats + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <section id="overview" class="grid gap-3 md:grid-cols-5">
            <StatCard label="Total" icon="i-lucide-database" value=move || stats().total.to_string() />
            <StatCard label="Ready" icon="i-lucide-play text-emerald-500" value=move || stats().ready.to_string() />
            <StatCard label="Scheduled" icon="i-lucide-clock text-amber-500" value=move || stats().scheduled.to_string() />
            <StatCard label="Locked" icon="i-lucide-lock text-cyan-500" value=move || stats().locked.to_string() />
            <StatCard label="Failed" icon="i-lucide-circle-alert text-rose-500" value=move || stats().failed.to_string() />
        </section>
    }
}

#[component]
fn StatCard(
    label: &'static str,
    icon: &'static str,
    value: impl Fn() -> String + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="gw-panel p-4">
            <div class="flex items-center justify-between">
                <span class="gw-muted text-sm">{label}</span>
                <span class=format!("{icon} h-4 w-4 gw-muted")></span>
            </div>
            <strong class="mt-2 block text-2xl">{move || value()}</strong>
        </div>
    }
}
