use leptos::prelude::*;

use super::api::refresh_data;
use super::browser::apply_theme;
use super::components::{ActiveWorkersPanel, Overview, QueuesPanel, WorkersPanel};
use super::filters::{filter_values, job_search_text, job_state, matches_column};
use super::modals::ModalView;
use super::types::{AdminClientConfig, JobState};

mod auth;
mod jobs;
mod maintenance;
mod sidebar;
mod signals;
mod state;
mod topbar;
use auth::AuthTokenPanel;
use jobs::JobsPanel;
use maintenance::MaintenancePanel;
use sidebar::Sidebar;
use signals::AdminSignals;
use topbar::Topbar;

#[component]
pub(super) fn AdminApp(config: AdminClientConfig) -> impl IntoView {
    let signals = AdminSignals::new();
    let refresh = signals.refresh();
    let jobs = signals.jobs;
    let overview = signals.overview;
    let selected_jobs = signals.selected_jobs;
    let selected_workers = signals.selected_workers;
    let active_state = signals.active_state;
    let search = signals.search;
    let task_filter = signals.task_filter;
    let queue_filter = signals.queue_filter;
    let key_filter = signals.key_filter;
    let worker_filter = signals.worker_filter;
    let modal = signals.modal;
    let toast = signals.toast;
    let theme = signals.theme;
    let accent = signals.accent;
    let compact = signals.compact;
    let auto_refresh_enabled = signals.auto_refresh_enabled;
    let auto_refresh_timer = signals.auto_refresh_timer.clone();

    let filtered_jobs = Memo::new(move |_| {
        let search = search.get().trim().to_lowercase();
        let active_state = active_state.get();
        let filters = [
            ("task_identifier", filter_values(&task_filter.get())),
            ("queue_name", filter_values(&queue_filter.get())),
            ("key", filter_values(&key_filter.get())),
            ("locked_by", filter_values(&worker_filter.get())),
        ];

        jobs.get()
            .into_iter()
            .filter(|job| {
                if active_state != JobState::All && job_state(job) != active_state {
                    return false;
                }
                if !search.is_empty() && !job_search_text(job).contains(&search) {
                    return false;
                }
                filters
                    .iter()
                    .all(|(column, values)| matches_column(job, column, values))
            })
            .collect::<Vec<_>>()
    });

    let selected_count = Memo::new(move |_| selected_jobs.get().len());
    let all_visible_selected = Memo::new(move |_| {
        let visible = filtered_jobs.get();
        let selected = selected_jobs.get();
        !visible.is_empty() && visible.iter().all(|job| selected.contains(&job.id))
    });
    let show_token_login = config.auth_mode.requires_token();

    Effect::new(move |_| {
        apply_theme(&theme.get(), &accent.get(), compact.get());
    });

    refresh_data(config.clone(), refresh);

    view! {
        <Sidebar config=config.clone() />

        <main class="gw-main">
            <Topbar
                config=config.clone()
                refresh=refresh
                theme=theme
                accent=accent
                compact=compact
                auto_refresh_enabled=auto_refresh_enabled
                auto_refresh_timer=auto_refresh_timer.clone()
            />

            <div class="gw-scroll">
                <AuthTokenPanel
                    show_token_login=show_token_login
                    config=config.clone()
                    refresh=refresh
                />

                <Overview stats=move || overview.get().stats />

                <JobsPanel
                    config=config.clone()
                    refresh=refresh
                    filtered_jobs=filtered_jobs
                    selected_count=selected_count
                    all_visible_selected=all_visible_selected
                    active_state=active_state
                    search=search
                    task_filter=task_filter
                    queue_filter=queue_filter
                    key_filter=key_filter
                    worker_filter=worker_filter
                    modal=modal
                />

                <section id="queues-workers" class="mt-4 grid gap-4 lg:grid-cols-2">
                    <QueuesPanel overview=overview queue_filter=queue_filter />
                    <div class="grid gap-4">
                        <ActiveWorkersPanel
                            config=config.clone()
                            refresh=refresh
                        />
                        <WorkersPanel
                            config=config.clone()
                            selected_workers=selected_workers
                            refresh=refresh
                        />
                    </div>
                </section>

                <MaintenancePanel
                    config=config.clone()
                    refresh=refresh
                />
            </div>
        </main>

        <ModalView
            modal=modal
            config=config
            refresh=refresh
        />
        <div class="gw-toast" data-open=move || toast.get().is_some().to_string()>{move || toast.get().unwrap_or_default()}</div>
    }
}
