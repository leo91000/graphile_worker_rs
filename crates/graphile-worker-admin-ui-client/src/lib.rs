#[inline(always)]
pub const fn manifest_dir() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

#[cfg(test)]
mod tests {
    use super::manifest_dir;

    #[test]
    fn manifest_dir_points_to_admin_ui_client_crate() {
        assert!(manifest_dir().ends_with("graphile-worker-admin-ui-client"));
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::RefCell;
    use std::rc::Rc;

    use chrono::{DateTime, Duration, NaiveDateTime, Utc};
    use gloo_net::http::{Request, RequestBuilder};
    use gloo_timers::callback::{Interval, Timeout};
    use leptos::mount::mount_to;
    use leptos::prelude::*;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::{spawn_local, JsFuture};
    use web_sys::{ClipboardEvent, Event, HtmlElement, HtmlTextAreaElement, RequestCredentials};

    #[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum JobState {
        #[default]
        All,
        Ready,
        Scheduled,
        Locked,
        Failed,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    struct ListedJob {
        id: i64,
        task_identifier: String,
        queue_name: Option<String>,
        payload: Value,
        priority: i16,
        run_at: DateTime<Utc>,
        attempts: i16,
        max_attempts: i16,
        last_error: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        key: Option<String>,
        locked_at: Option<DateTime<Utc>>,
        locked_by: Option<String>,
        revision: i32,
        flags: Option<Value>,
        is_available: bool,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    struct JobStats {
        total: i64,
        ready: i64,
        scheduled: i64,
        locked: i64,
        failed: i64,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct QueueRow {
        id: i32,
        queue_name: String,
        locked_at: Option<DateTime<Utc>>,
        locked_by: Option<String>,
        job_count: i64,
        ready_count: i64,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct LockedWorkerRow {
        worker_id: String,
        locked_jobs: i64,
        locked_queues: i64,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct AddJobRequest {
        identifier: String,
        #[serde(default)]
        payload: Value,
        queue: Option<String>,
        run_at: Option<DateTime<Utc>>,
        max_attempts: Option<i16>,
        key: Option<String>,
        job_key_mode: Option<JobKeyModeRequest>,
        priority: Option<i16>,
        flags: Option<Vec<String>>,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum JobKeyModeRequest {
        Replace,
        PreserveRunAt,
        UnsafeDedupe,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct JobActionRequest {
        action: JobAction,
        ids: Vec<i64>,
        reason: Option<String>,
        run_at: Option<DateTime<Utc>>,
        priority: Option<i16>,
        attempts: Option<i16>,
        max_attempts: Option<i16>,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum JobAction {
        Complete,
        Fail,
        RunNow,
        Reschedule,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct RemoveJobByKeyRequest {
        key: String,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct MaintenanceRequest {
        action: MaintenanceAction,
        #[serde(default)]
        cleanup_tasks: Vec<CleanupTaskName>,
        #[serde(default)]
        worker_ids: Vec<String>,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum MaintenanceAction {
        Migrate,
        Cleanup,
        ForceUnlock,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum CleanupTaskName {
        DeletePermanentlyFailedJobs,
        GcTaskIdentifiers,
        GcJobQueues,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct OverviewResponse {
        stats: JobStats,
        queues: Vec<QueueRow>,
        workers: Vec<LockedWorkerRow>,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct ListJobsResponse {
        jobs: Vec<ListedJob>,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct JobActionResponse {
        message: String,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct MessageResponse {
        message: String,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct ErrorResponse {
        error: String,
    }

    #[derive(Clone, Debug)]
    struct AdminClientConfig {
        auth_mode: String,
        auth_header: String,
        csrf: String,
        csrf_header: String,
        read_only: bool,
        schema: String,
    }

    #[derive(Clone, Debug)]
    enum Modal {
        AddJob,
        FailJobs,
        Reschedule,
        RemoveKey,
        JobDetails(ListedJob),
    }

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

    fn data(root: &HtmlElement, key: &str) -> String {
        root.dataset().get(key).unwrap_or_default()
    }

    #[component]
    fn AdminApp(config: AdminClientConfig) -> impl IntoView {
        let jobs = RwSignal::new(Vec::<ListedJob>::new());
        let overview = RwSignal::new(OverviewResponse {
            stats: Default::default(),
            queues: Vec::new(),
            workers: Vec::new(),
        });
        let selected_jobs = RwSignal::new(Vec::<i64>::new());
        let selected_workers = RwSignal::new(Vec::<String>::new());
        let active_state = RwSignal::new(JobState::All);
        let search = RwSignal::new(String::new());
        let task_filter = RwSignal::new(String::new());
        let queue_filter = RwSignal::new(String::new());
        let key_filter = RwSignal::new(String::new());
        let worker_filter = RwSignal::new(String::new());
        let limit = RwSignal::new(100_i64);
        let modal = RwSignal::new(None::<Modal>);
        let toast = RwSignal::new(None::<String>);
        let token = RwSignal::new(storage_get("gw-admin-token").unwrap_or_default());
        let theme =
            RwSignal::new(storage_get("gw-admin-theme").unwrap_or_else(|| "system".to_string()));
        let accent =
            RwSignal::new(storage_get("gw-admin-accent").unwrap_or_else(|| "cyan".to_string()));
        let compact = RwSignal::new(storage_get("gw-admin-density").as_deref() == Some("compact"));
        let auto_refresh_timer = Rc::new(RefCell::new(None::<Interval>));

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
        let show_token_login = config.auth_mode == "bearer" || config.auth_mode == "header";

        Effect::new(move |_| {
            apply_theme(&theme.get(), &accent.get(), compact.get());
        });

        refresh_data(
            config.clone(),
            token,
            limit,
            overview,
            jobs,
            selected_jobs,
            toast,
        );

        let refresh_click = {
            let config = config.clone();
            move |_| {
                refresh_data(
                    config.clone(),
                    token,
                    limit,
                    overview,
                    jobs,
                    selected_jobs,
                    toast,
                );
            }
        };

        let copy_selected_json = move |_| {
            let rows = selected_rows(jobs, selected_jobs);
            copy_to_clipboard(
                serde_json::to_string_pretty(&rows).unwrap_or_default(),
                "Copied selected JSON",
                toast,
            );
        };
        let copy_selected_csv = move |_| {
            copy_to_clipboard(
                selected_csv(&selected_rows(jobs, selected_jobs)),
                "Copied selected CSV",
                toast,
            );
        };

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
                    <a class="gw-tab" href="#jobs" aria-selected="true"><span class="i-lucide-list-checks h-4 w-4"></span>"Jobs"</a>
                    <a class="gw-tab" href="#queues"><span class="i-lucide-git-branch h-4 w-4"></span>"Queues"</a>
                    <a class="gw-tab" href="#workers"><span class="i-lucide-hard-drive h-4 w-4"></span>"Workers"</a>
                    <a class="gw-tab" href="#maintenance"><span class="i-tabler-tool h-4 w-4"></span>"Maintenance"</a>
                </nav>

                <div class="mt-auto grid gap-3 rounded-lg border p-3 text-xs" style="border-color: rgb(var(--border));">
                    <div class="flex items-center justify-between"><span class="gw-muted">"Schema"</span><span class="font-mono">{config.schema.clone()}</span></div>
                    <div class="flex items-center justify-between"><span class="gw-muted">"Auth"</span><span class="gw-pill">{config.auth_mode.clone()}</span></div>
                    <div class="flex items-center justify-between"><span class="gw-muted">"Writes"</span><span class="gw-pill">{if config.read_only { "read only" } else { "enabled" }}</span></div>
                </div>
            </aside>

            <main class="gw-main">
                <header class="gw-topbar">
                    <div class="min-w-0">
                        <p class="gw-muted text-xs">"PostgreSQL-backed queue control plane"</p>
                        <h2 class="truncate text-lg font-semibold">"Jobs, queues, and workers"</h2>
                    </div>
                    <div class="flex flex-wrap items-center justify-end gap-2">
                        <select
                            id="theme-select"
                            name="theme"
                            class="gw-input w-32"
                            aria-label="Theme"
                            prop:value=move || theme.get()
                            on:change=move |event| {
                                let value = event_target_value(&event);
                                storage_set("gw-admin-theme", &value);
                                theme.set(value);
                            }
                        >
                            <option value="system">"System"</option>
                            <option value="light">"Light"</option>
                            <option value="dark">"Dark"</option>
                        </select>
                        <select
                            id="accent-select"
                            name="accent"
                            class="gw-input w-32"
                            aria-label="Accent"
                            prop:value=move || accent.get()
                            on:change=move |event| {
                                let value = event_target_value(&event);
                                storage_set("gw-admin-accent", &value);
                                accent.set(value);
                            }
                        >
                            <option value="cyan">"Cyan"</option>
                            <option value="emerald">"Emerald"</option>
                            <option value="violet">"Violet"</option>
                            <option value="amber">"Amber"</option>
                        </select>
                        <button
                            class="gw-btn"
                            type="button"
                            title="Toggle density"
                            on:click=move |_| {
                                let next = !compact.get_untracked();
                                storage_set("gw-admin-density", if next { "compact" } else { "comfortable" });
                                compact.set(next);
                            }
                        >
                            <span class="i-lucide-align-justify h-4 w-4"></span>
                        </button>
                        <label class="gw-btn cursor-pointer">
                            <input
                                id="auto-refresh"
                                name="auto_refresh"
                                class="h-4 w-4"
                                type="checkbox"
                                on:change={
                                    let config = config.clone();
                                    let auto_refresh_timer = auto_refresh_timer.clone();
                                    move |event| {
                                        if event_target_checked(&event) {
                                            let config = config.clone();
                                            let handle = Interval::new(5000, move || {
                                                refresh_data(
                                                    config.clone(),
                                                    token,
                                                    limit,
                                                    overview,
                                                    jobs,
                                                    selected_jobs,
                                                    toast,
                                                );
                                            });
                                            *auto_refresh_timer.borrow_mut() = Some(handle);
                                        } else {
                                            *auto_refresh_timer.borrow_mut() = None;
                                        }
                                    }
                                }
                            />
                            <span class="text-sm">"Auto"</span>
                        </label>
                        <button class="gw-btn gw-btn-primary" type="button" on:click=refresh_click>
                            <span class="i-lucide-refresh-cw h-4 w-4"></span>
                            "Refresh"
                        </button>
                    </div>
                </header>

                <div class="gw-scroll">
                    <section class=if show_token_login { "gw-panel mb-4 p-4" } else { "hidden" }>
                        <div class="flex flex-wrap items-end gap-3">
                            <div class="min-w-72 flex-1">
                                <label class="mb-1 block text-sm font-medium" for="auth-token">"API token"</label>
                                <input
                                    id="auth-token"
                                    class="gw-input w-full"
                                    type="password"
                                    autocomplete="current-password"
                                    prop:value=move || token.get()
                                    on:input=move |event| token.set(event_target_value(&event))
                                />
                            </div>
                            <button
                                class="gw-btn gw-btn-primary"
                                type="button"
                                on:click={
                                    let config = config.clone();
                                    move |_| {
                                        storage_set("gw-admin-token", &token.get_untracked());
                                        refresh_data(
                                            config.clone(),
                                            token,
                                            limit,
                                            overview,
                                            jobs,
                                            selected_jobs,
                                            toast,
                                        );
                                    }
                                }
                            >
                                <span class="i-lucide-key-round h-4 w-4"></span>
                                "Use token"
                            </button>
                            <button class="gw-btn" type="button" on:click=move |_| {
                                storage_remove("gw-admin-token");
                                token.set(String::new());
                            }>"Clear"</button>
                        </div>
                    </section>

                    <Overview stats=move || overview.get().stats />

                    <section id="jobs" class="gw-panel mt-4">
                        <div class="flex flex-wrap items-center justify-between gap-3 border-b p-3" style="border-color: rgb(var(--border));">
                            <div class="flex flex-wrap items-center gap-2" role="tablist" aria-label="Job state">
                                <StateTab label="All" state=JobState::All active_state=active_state />
                                <StateTab label="Ready" state=JobState::Ready active_state=active_state />
                                <StateTab label="Scheduled" state=JobState::Scheduled active_state=active_state />
                                <StateTab label="Locked" state=JobState::Locked active_state=active_state />
                                <StateTab label="Failed" state=JobState::Failed active_state=active_state />
                            </div>
                            <div class="flex flex-wrap items-center gap-2">
                                <button class="gw-btn gw-btn-primary" type="button" disabled=config.read_only on:click=move |_| modal.set(Some(Modal::AddJob))>
                                    <span class="i-lucide-plus h-4 w-4"></span>"Add"
                                </button>
                                <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 on:click=copy_selected_json>
                                    <span class="i-lucide-copy h-4 w-4"></span>"JSON"
                                </button>
                                <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 on:click=copy_selected_csv>
                                    <span class="i-lucide-clipboard h-4 w-4"></span>"CSV"
                                </button>
                            </div>
                        </div>

                        <div class="grid gap-2 border-b p-3 lg:grid-cols-[minmax(240px,1fr)_repeat(4,minmax(120px,180px))_110px]" style="border-color: rgb(var(--border));">
                            <input id="global-search" name="global_search" class="gw-input" type="search" placeholder="Search id, task, queue, key, worker, payload..." prop:value=move || search.get() on:input=move |event| search.set(event_target_value(&event)) />
                            <ColumnFilter name="task_filter" placeholder="Task filter" value=task_filter />
                            <ColumnFilter name="queue_filter" placeholder="Queue filter" value=queue_filter />
                            <ColumnFilter name="key_filter" placeholder="Key filter" value=key_filter />
                            <ColumnFilter name="worker_filter" placeholder="Worker filter" value=worker_filter />
                            <select
                                id="limit-select"
                                name="limit"
                                class="gw-input"
                                prop:value=move || limit.get().to_string()
                                on:change={
                                    let config = config.clone();
                                    move |event| {
                                        limit.set(event_target_value(&event).parse().unwrap_or(100));
                                        refresh_data(
                                            config.clone(),
                                            token,
                                            limit,
                                            overview,
                                            jobs,
                                            selected_jobs,
                                            toast,
                                        );
                                    }
                                }
                            >
                                <option value="50">"50"</option>
                                <option value="100">"100"</option>
                                <option value="250">"250"</option>
                                <option value="500">"500"</option>
                            </select>
                        </div>

                        <div class="flex flex-wrap items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                            <span class="gw-pill">{move || format!("{} selected", selected_count.get())}</span>
                            <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click={
                                let config = config.clone();
                                move |_| post_job_action(
                                    config.clone(),
                                    token,
                                    JobActionRequest {
                                        action: JobAction::Complete,
                                        ids: selected_jobs.get_untracked(),
                                        reason: None,
                                        run_at: None,
                                        priority: None,
                                        attempts: None,
                                        max_attempts: None,
                                    },
                                    overview,
                                    jobs,
                                    selected_jobs,
                                    limit,
                                    toast,
                                )
                            }>
                                <span class="i-lucide-check h-4 w-4"></span>"Complete"
                            </button>
                            <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click={
                                let config = config.clone();
                                move |_| post_job_action(
                                    config.clone(),
                                    token,
                                    JobActionRequest {
                                        action: JobAction::RunNow,
                                        ids: selected_jobs.get_untracked(),
                                        reason: None,
                                        run_at: None,
                                        priority: None,
                                        attempts: None,
                                        max_attempts: None,
                                    },
                                    overview,
                                    jobs,
                                    selected_jobs,
                                    limit,
                                    toast,
                                )
                            }>
                                <span class="i-lucide-play h-4 w-4"></span>"Run now"
                            </button>
                            <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click=move |_| modal.set(Some(Modal::Reschedule))>
                                <span class="i-lucide-calendar-clock h-4 w-4"></span>"Reschedule"
                            </button>
                            <button class="gw-btn gw-btn-danger" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click=move |_| modal.set(Some(Modal::FailJobs))>
                                <span class="i-lucide-ban h-4 w-4"></span>"Fail"
                            </button>
                            <button class="gw-btn" type="button" disabled=config.read_only on:click=move |_| modal.set(Some(Modal::RemoveKey))>
                                <span class="i-lucide-key-x h-4 w-4"></span>"Remove by key"
                            </button>
                        </div>

                        <div class="max-h-[58vh] overflow-auto">
                            <table class="gw-table">
                                <thead>
                                    <tr>
                                        <th>
                                            <input
                                                id="select-all-jobs"
                                                name="select_all_jobs"
                                                type="checkbox"
                                                class="h-4 w-4"
                                                prop:checked=move || all_visible_selected.get()
                                                on:change=move |event| {
                                                    let visible_ids = filtered_jobs.get_untracked().into_iter().map(|job| job.id).collect::<Vec<_>>();
                                                    selected_jobs.update(|selected| {
                                                        if event_target_checked(&event) {
                                                            for id in visible_ids {
                                                                if !selected.contains(&id) {
                                                                    selected.push(id);
                                                                }
                                                            }
                                                        } else {
                                                            selected.retain(|id| !visible_ids.contains(id));
                                                        }
                                                    });
                                                }
                                            />
                                        </th>
                                        <th>"ID"</th><th>"Task"</th><th>"Queue"</th><th>"State"</th><th>"Run at"</th><th>"Attempts"</th><th>"Priority"</th><th>"Key"</th><th>"Payload"</th><th>"Error"</th><th>"Actions"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {move || if filtered_jobs.get().is_empty() {
                                        view! { <tr><td colspan="12" class="py-8 text-center gw-muted">"No jobs match the current filters."</td></tr> }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                    <For
                                        each=move || filtered_jobs.get()
                                        key=|job| job.id
                                        children=move |job| {
                                            view! {
                                                <JobRow
                                                    job=job
                                                    selected_jobs=selected_jobs
                                                    modal=modal
                                                    toast=toast
                                                />
                                            }
                                        }
                                    />
                                </tbody>
                            </table>
                        </div>
                    </section>

                    <section id="queues-workers" class="mt-4 grid gap-4 lg:grid-cols-2">
                        <QueuesPanel overview=overview queue_filter=queue_filter />
                        <WorkersPanel
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            selected_workers=selected_workers
                            limit=limit
                            toast=toast
                        />
                    </section>

                    <section id="maintenance" class="gw-panel mt-4 p-4">
                        <div class="flex flex-wrap items-center justify-between gap-3">
                            <div>
                                <h3 class="font-semibold">"Maintenance"</h3>
                                <p class="gw-muted text-sm">"Run migrations, cleanup orphaned queue metadata, and recover abandoned locks."</p>
                            </div>
                            <div class="flex flex-wrap gap-2">
                                <button class="gw-btn" type="button" disabled=config.read_only on:click={
                                    let config = config.clone();
                                    move |_| post_maintenance(
                                        config.clone(),
                                        token,
                                        MaintenanceRequest {
                                            action: MaintenanceAction::Migrate,
                                            cleanup_tasks: Vec::new(),
                                            worker_ids: Vec::new(),
                                        },
                                        overview,
                                        jobs,
                                        selected_jobs,
                                        limit,
                                        toast,
                                    )
                                }>
                                    <span class="i-lucide-database-zap h-4 w-4"></span>"Migrate"
                                </button>
                                <button class="gw-btn" type="button" disabled=config.read_only on:click={
                                    let config = config.clone();
                                    move |_| post_maintenance(
                                        config.clone(),
                                        token,
                                        MaintenanceRequest {
                                            action: MaintenanceAction::Cleanup,
                                            cleanup_tasks: vec![
                                                CleanupTaskName::DeletePermanentlyFailedJobs,
                                                CleanupTaskName::GcTaskIdentifiers,
                                                CleanupTaskName::GcJobQueues,
                                            ],
                                            worker_ids: Vec::new(),
                                        },
                                        overview,
                                        jobs,
                                        selected_jobs,
                                        limit,
                                        toast,
                                    )
                                }>
                                    <span class="i-lucide-sparkles h-4 w-4"></span>"Cleanup"
                                </button>
                            </div>
                        </div>
                    </section>
                </div>
            </main>

            <ModalView
                modal=modal
                config=config
                token=token
                overview=overview
                jobs=jobs
                selected_jobs=selected_jobs
                limit=limit
                toast=toast
            />
            <div class="gw-toast" data-open=move || toast.get().is_some().to_string()>{move || toast.get().unwrap_or_default()}</div>
        }
    }

    #[component]
    fn Overview(stats: impl Fn() -> JobStats + Copy + Send + Sync + 'static) -> impl IntoView {
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

    #[component]
    fn StateTab(
        label: &'static str,
        state: JobState,
        active_state: RwSignal<JobState>,
    ) -> impl IntoView {
        view! {
            <button
                class="gw-tab"
                aria-selected=move || (active_state.get() == state).to_string()
                type="button"
                on:click=move |_| active_state.set(state)
            >
                {label}
            </button>
        }
    }

    #[component]
    fn ColumnFilter(
        name: &'static str,
        placeholder: &'static str,
        value: RwSignal<String>,
    ) -> impl IntoView {
        view! {
            <input
                class="gw-input column-filter"
                name=name
                type="search"
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |event| value.set(event_target_value(&event))
                on:paste=move |event: ClipboardEvent| {
                    let pasted = event
                        .clipboard_data()
                        .and_then(|data| data.get_data("text").ok())
                        .unwrap_or_default();
                    if pasted.contains('\n') || pasted.contains('\r') || pasted.contains('\t') || pasted.contains(',') {
                        event.prevent_default();
                        value.set(normalize_filter_paste(&pasted));
                    }
                }
            />
        }
    }

    #[component]
    fn JobRow(
        job: ListedJob,
        selected_jobs: RwSignal<Vec<i64>>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        let id = job.id;
        let row_state = job_state(&job);
        let queue = job.queue_name.clone();
        let key = job.key.clone();
        let payload = stringify_value(&job.payload);
        let last_error = job.last_error.clone().unwrap_or_default();
        let job_for_details = job.clone();

        view! {
            <tr>
                <td>
                    <input
                        class="h-4 w-4"
                        type="checkbox"
                        name="selected_jobs"
                        aria-label=format!("Select job {id}")
                        prop:checked=move || selected_jobs.get().contains(&id)
                        on:change=move |event| {
                            selected_jobs.update(|selected| {
                                if event_target_checked(&event) {
                                    if !selected.contains(&id) {
                                        selected.push(id);
                                    }
                                } else {
                                    selected.retain(|candidate| *candidate != id);
                                }
                            });
                        }
                    />
                </td>
                <td class="font-mono">{id}</td>
                <td><button class="font-medium hover:underline" type="button" on:click={
                    let task = job.task_identifier.clone();
                    move |_| copy_to_clipboard(task.clone(), "Copied task_identifier", toast)
                }>{job.task_identifier.clone()}</button></td>
                <td>{queue.clone().map(|queue| view! {
                    <button class="hover:underline" type="button" on:click={
                        let queue = queue.clone();
                        move |_| copy_to_clipboard(queue.clone(), "Copied queue_name", toast)
                    }>{queue.clone()}</button>
                }.into_any()).unwrap_or_else(|| view! { <span class="gw-muted">"default"</span> }.into_any())}</td>
                <td><span class=format!("gw-pill {}", state_color(row_state))>{state_label(row_state)}</span></td>
                <td class="whitespace-nowrap">{format_date(job.run_at)}</td>
                <td>{format!("{}/{}", job.attempts, job.max_attempts)}</td>
                <td>{job.priority}</td>
                <td>{key.clone().map(|key| view! {
                    <button class="font-mono text-xs hover:underline" type="button" on:click={
                        let key = key.clone();
                        move |_| copy_to_clipboard(key.clone(), "Copied key", toast)
                    }>{short(&key, 28)}</button>
                }.into_any()).unwrap_or_else(|| ().into_any())}</td>
                <td><button class="max-w-72 text-left font-mono text-xs hover:underline" type="button" on:click={
                    let payload = payload.clone();
                    move |_| copy_to_clipboard(payload.clone(), "Copied payload", toast)
                }>{short(&payload, 90)}</button></td>
                <td><button class="max-w-56 text-left text-xs text-rose-600 hover:underline dark:text-rose-300" type="button" on:click={
                    let last_error = last_error.clone();
                    move |_| copy_to_clipboard(last_error.clone(), "Copied last_error", toast)
                }>{short(&last_error, 70)}</button></td>
                <td>
                    <button class="gw-btn h-8 px-2" type="button" title="View details" on:click=move |_| modal.set(Some(Modal::JobDetails(job_for_details.clone())))>
                        <span class="i-lucide-eye h-4 w-4"></span>
                    </button>
                </td>
            </tr>
        }
    }

    #[component]
    fn QueuesPanel(
        overview: RwSignal<OverviewResponse>,
        queue_filter: RwSignal<String>,
    ) -> impl IntoView {
        view! {
            <div id="queues" class="gw-panel">
                <div class="flex items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                    <span class="i-lucide-git-branch h-4 w-4"></span>
                    <h3 class="font-semibold">"Queues"</h3>
                </div>
                <div class="divide-y" style="border-color: rgb(var(--border));">
                    {move || if overview.get().queues.is_empty() {
                        view! { <div class="p-4 gw-muted">"No queues yet."</div> }.into_any()
                    } else {
                        ().into_any()
                    }}
                    <For
                        each=move || overview.get().queues
                        key=|queue| queue.id
                        children=move |queue| {
                            let queue_name = queue.queue_name.clone();
                            view! {
                                <div class="flex items-center justify-between gap-3 p-3">
                                    <div class="min-w-0">
                                        <button class="truncate font-medium hover:underline" type="button" on:click=move |_| {
                                            queue_filter.set(queue_name.clone());
                                            if let Some(window) = web_sys::window() {
                                                let _ = window.location().set_hash("jobs");
                                            }
                                        }>{queue.queue_name.clone()}</button>
                                        <p class="gw-muted text-xs">{queue.locked_by.clone().map(|worker| format!("locked by {worker}")).unwrap_or_else(|| "unlocked".to_string())}</p>
                                    </div>
                                    <div class="flex gap-2">
                                        <span class="gw-pill">{format!("{} ready", queue.ready_count)}</span>
                                        <span class="gw-pill">{format!("{} total", queue.job_count)}</span>
                                    </div>
                                </div>
                            }
                        }
                    />
                </div>
            </div>
        }
    }

    #[component]
    fn WorkersPanel(
        config: AdminClientConfig,
        token: RwSignal<String>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        selected_workers: RwSignal<Vec<String>>,
        limit: RwSignal<i64>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        view! {
            <div id="workers" class="gw-panel">
                <div class="flex items-center justify-between gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                    <div class="flex items-center gap-2">
                        <span class="i-lucide-hard-drive h-4 w-4"></span>
                        <h3 class="font-semibold">"Workers"</h3>
                    </div>
                    <button class="gw-btn" type="button" disabled=config.read_only on:click={
                        let config = config.clone();
                        move |_| {
                            let worker_ids = selected_workers.get_untracked();
                            if worker_ids.is_empty() {
                                show_toast(toast, "Select at least one worker");
                                return;
                            }
                            post_maintenance(
                                config.clone(),
                                token,
                                MaintenanceRequest {
                                    action: MaintenanceAction::ForceUnlock,
                                    cleanup_tasks: Vec::new(),
                                    worker_ids,
                                },
                                overview,
                                jobs,
                                selected_jobs,
                                limit,
                                toast,
                            );
                        }
                    }>
                        <span class="i-lucide-unlock h-4 w-4"></span>"Force unlock"
                    </button>
                </div>
                <div class="divide-y" style="border-color: rgb(var(--border));">
                    {move || if overview.get().workers.is_empty() {
                        view! { <div class="p-4 gw-muted">"No active locks."</div> }.into_any()
                    } else {
                        ().into_any()
                    }}
                    <For
                        each=move || overview.get().workers
                        key=|worker| worker.worker_id.clone()
                        children=move |worker| {
                            let worker_id = worker.worker_id.clone();
                            let worker_id_for_checked = worker_id.clone();
                            let worker_id_for_change = worker_id.clone();
                            view! {
                                <label class="flex cursor-pointer items-center justify-between gap-3 p-3">
                                    <span class="min-w-0">
                                        <span class="block truncate font-mono text-sm">{worker.worker_id.clone()}</span>
                                        <span class="gw-muted text-xs">{format!("{} jobs · {} queues", worker.locked_jobs, worker.locked_queues)}</span>
                                    </span>
                                    <input
                                        class="h-4 w-4"
                                        type="checkbox"
                                        name="selected_workers"
                                        aria-label=format!("Select worker {}", worker.worker_id)
                                        prop:checked=move || selected_workers.get().contains(&worker_id_for_checked)
                                        on:change=move |event| {
                                            selected_workers.update(|selected| {
                                                if event_target_checked(&event) {
                                                    if !selected.contains(&worker_id_for_change) {
                                                        selected.push(worker_id_for_change.clone());
                                                    }
                                                } else {
                                                    selected.retain(|candidate| candidate != &worker_id_for_change);
                                                }
                                            });
                                        }
                                    />
                                </label>
                            }
                        }
                    />
                </div>
            </div>
        }
    }

    #[component]
    fn ModalView(
        modal: RwSignal<Option<Modal>>,
        config: AdminClientConfig,
        token: RwSignal<String>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        view! {
            <div class="gw-modal" data-open=move || modal.get().is_some().to_string() role="dialog" aria-modal="true">
                <div class="gw-dialog">
                    <div class="mb-4 flex items-center justify-between gap-3">
                        <h3 class="text-lg font-semibold">{move || modal_title(&modal.get())}</h3>
                        <button class="gw-btn" type="button" aria-label="Close" on:click=move |_| modal.set(None)>
                            <span class="i-lucide-x h-4 w-4"></span>
                        </button>
                    </div>
                    {move || match modal.get() {
                        Some(Modal::AddJob) => view! {
                            <AddJobModal
                                config=config.clone()
                                token=token
                                overview=overview
                                jobs=jobs
                                selected_jobs=selected_jobs
                                limit=limit
                                modal=modal
                                toast=toast
                            />
                        }.into_any(),
                        Some(Modal::FailJobs) => view! {
                            <FailJobsModal
                                config=config.clone()
                                token=token
                                overview=overview
                                jobs=jobs
                                selected_jobs=selected_jobs
                                limit=limit
                                modal=modal
                                toast=toast
                            />
                        }.into_any(),
                        Some(Modal::Reschedule) => view! {
                            <RescheduleModal
                                config=config.clone()
                                token=token
                                overview=overview
                                jobs=jobs
                                selected_jobs=selected_jobs
                                limit=limit
                                modal=modal
                                toast=toast
                            />
                        }.into_any(),
                        Some(Modal::RemoveKey) => view! {
                            <RemoveKeyModal
                                config=config.clone()
                                token=token
                                overview=overview
                                jobs=jobs
                                selected_jobs=selected_jobs
                                limit=limit
                                modal=modal
                                toast=toast
                            />
                        }.into_any(),
                        Some(Modal::JobDetails(job)) => view! { <JobDetailsModal job=job toast=toast /> }.into_any(),
                        None => ().into_any(),
                    }}
                </div>
            </div>
        }
    }

    #[component]
    fn AddJobModal(
        config: AdminClientConfig,
        token: RwSignal<String>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        let identifier = RwSignal::new(String::new());
        let queue = RwSignal::new(String::new());
        let key = RwSignal::new(String::new());
        let key_mode = RwSignal::new(String::new());
        let priority = RwSignal::new(String::new());
        let max_attempts = RwSignal::new(String::new());
        let run_at = RwSignal::new(String::new());
        let payload = RwSignal::new("{\"hello\":\"world\"}".to_string());
        let flags = RwSignal::new(String::new());

        view! {
            <form class="grid gap-3" on:submit=move |event| {
                event.prevent_default();
                let payload_value = match serde_json::from_str::<Value>(&payload.get_untracked()) {
                    Ok(payload) => payload,
                    Err(error) => {
                        show_toast(toast, format!("Invalid JSON: {error}"));
                        return;
                    }
                };
                let request = AddJobRequest {
                    identifier: identifier.get_untracked(),
                    payload: payload_value,
                    queue: optional_string(queue.get_untracked()),
                    run_at: datetime_local_to_utc(&run_at.get_untracked()),
                    max_attempts: optional_i16(max_attempts.get_untracked()),
                    key: optional_string(key.get_untracked()),
                    job_key_mode: match key_mode.get_untracked().as_str() {
                        "replace" => Some(JobKeyModeRequest::Replace),
                        "preserve-run-at" => Some(JobKeyModeRequest::PreserveRunAt),
                        "unsafe-dedupe" => Some(JobKeyModeRequest::UnsafeDedupe),
                        _ => None,
                    },
                    priority: optional_i16(priority.get_untracked()),
                    flags: optional_csv(flags.get_untracked()),
                };
                post_add_job(config.clone(), token, request, overview, jobs, selected_jobs, limit, modal, toast);
            }>
                <div class="grid gap-3 md:grid-cols-2">
                    <TextInput label="Task identifier" value=identifier required=true />
                    <TextInput label="Queue" value=queue required=false />
                    <TextInput label="Key" value=key required=false />
                    <label class="grid gap-1 text-sm" for="key-mode">"Key mode"
                        <select id="key-mode" name="job_key_mode" class="gw-input" prop:value=move || key_mode.get() on:change=move |event| key_mode.set(event_target_value(&event))>
                            <option value="">"Default"</option>
                            <option value="replace">"Replace"</option>
                            <option value="preserve-run-at">"Preserve run_at"</option>
                            <option value="unsafe-dedupe">"Unsafe dedupe"</option>
                        </select>
                    </label>
                    <TextInput label="Priority" value=priority input_type="number" required=false />
                    <TextInput label="Max attempts" value=max_attempts input_type="number" required=false />
                    <TextInput label="Run at" value=run_at input_type="datetime-local" required=false class="grid gap-1 text-sm md:col-span-2" />
                </div>
                <label class="grid gap-1 text-sm" for="job-payload">"Payload JSON"
                    <textarea id="job-payload" name="payload" class="gw-textarea font-mono" prop:value=move || payload.get() on:input=move |event| payload.set(textarea_value(&event))></textarea>
                </label>
                <TextInput label="Flags, comma-separated" value=flags required=false />
                <ModalButtons modal=modal danger=false submit_label="Add job" submit_icon="i-lucide-plus" />
            </form>
        }
    }

    #[component]
    fn FailJobsModal(
        config: AdminClientConfig,
        token: RwSignal<String>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        let reason = RwSignal::new("Marked failed from Graphile Worker admin UI".to_string());
        view! {
            <form class="grid gap-3" on:submit=move |event| {
                event.prevent_default();
                post_job_action(
                    config.clone(),
                    token,
                    JobActionRequest {
                        action: JobAction::Fail,
                        ids: selected_jobs.get_untracked(),
                        reason: optional_string(reason.get_untracked()),
                        run_at: None,
                        priority: None,
                        attempts: None,
                        max_attempts: None,
                    },
                    overview,
                    jobs,
                    selected_jobs,
                    limit,
                    toast,
                );
                modal.set(None);
            }>
                <p class="gw-muted text-sm">{move || format!("This permanently fails {} selected job(s).", selected_jobs.get().len())}</p>
                <label class="grid gap-1 text-sm" for="fail-reason">"Reason"
                    <textarea id="fail-reason" name="reason" class="gw-textarea" prop:value=move || reason.get() on:input=move |event| reason.set(textarea_value(&event))></textarea>
                </label>
                <ModalButtons modal=modal danger=true submit_label="Fail jobs" submit_icon="i-lucide-ban" />
            </form>
        }
    }

    #[component]
    fn RescheduleModal(
        config: AdminClientConfig,
        token: RwSignal<String>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        let run_at = RwSignal::new(String::new());
        let priority = RwSignal::new(String::new());
        let attempts = RwSignal::new(String::new());
        let max_attempts = RwSignal::new(String::new());
        view! {
            <form class="grid gap-3" on:submit=move |event| {
                event.prevent_default();
                post_job_action(
                    config.clone(),
                    token,
                    JobActionRequest {
                        action: JobAction::Reschedule,
                        ids: selected_jobs.get_untracked(),
                        reason: None,
                        run_at: datetime_local_to_utc(&run_at.get_untracked()),
                        priority: optional_i16(priority.get_untracked()),
                        attempts: optional_i16(attempts.get_untracked()),
                        max_attempts: optional_i16(max_attempts.get_untracked()),
                    },
                    overview,
                    jobs,
                    selected_jobs,
                    limit,
                    toast,
                );
                modal.set(None);
            }>
                <div class="grid gap-3 md:grid-cols-2">
                    <TextInput label="Run at" value=run_at input_type="datetime-local" required=false />
                    <TextInput label="Priority" value=priority input_type="number" required=false />
                    <TextInput label="Attempts" value=attempts input_type="number" required=false />
                    <TextInput label="Max attempts" value=max_attempts input_type="number" required=false />
                </div>
                <ModalButtons modal=modal danger=false submit_label="Reschedule" submit_icon="i-lucide-calendar-clock" />
            </form>
        }
    }

    #[component]
    fn RemoveKeyModal(
        config: AdminClientConfig,
        token: RwSignal<String>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) -> impl IntoView {
        let key = RwSignal::new(String::new());
        view! {
            <form class="grid gap-3" on:submit=move |event| {
                event.prevent_default();
                post_remove_key(
                    config.clone(),
                    token,
                    RemoveJobByKeyRequest { key: key.get_untracked() },
                    overview,
                    jobs,
                    selected_jobs,
                    limit,
                    modal,
                    toast,
                );
            }>
                <TextInput label="Job key" value=key required=true />
                <ModalButtons modal=modal danger=true submit_label="Remove" submit_icon="i-lucide-key-x" />
            </form>
        }
    }

    #[component]
    fn JobDetailsModal(job: ListedJob, toast: RwSignal<Option<String>>) -> impl IntoView {
        let row_state = state_label(job_state(&job));
        let job_json = serde_json::to_string_pretty(&job).unwrap_or_default();
        view! {
            <div class="grid gap-3">
                <div class="grid gap-3 md:grid-cols-3">
                    <div class="gw-panel p-3"><span class="gw-muted text-xs">"Task"</span><strong class="block">{job.task_identifier.clone()}</strong></div>
                    <div class="gw-panel p-3"><span class="gw-muted text-xs">"Queue"</span><strong class="block">{job.queue_name.clone().unwrap_or_else(|| "default".to_string())}</strong></div>
                    <div class="gw-panel p-3"><span class="gw-muted text-xs">"State"</span><strong class="block">{row_state}</strong></div>
                </div>
                <label class="grid gap-1 text-sm" for="job-detail-payload">"Payload"
                    <textarea id="job-detail-payload" name="job_detail_payload" class="gw-textarea font-mono" readonly>{serde_json::to_string_pretty(&job.payload).unwrap_or_default()}</textarea>
                </label>
                <label class="grid gap-1 text-sm" for="job-detail-last-error">"Last error"
                    <textarea id="job-detail-last-error" name="job_detail_last_error" class="gw-textarea font-mono" readonly>{job.last_error.clone().unwrap_or_default()}</textarea>
                </label>
                <div class="flex justify-end gap-2">
                    <button class="gw-btn" type="button" on:click=move |_| copy_to_clipboard(job_json.clone(), "Copied job JSON", toast)>
                        <span class="i-lucide-copy h-4 w-4"></span>"Copy JSON"
                    </button>
                </div>
            </div>
        }
    }

    #[component]
    fn TextInput(
        label: &'static str,
        value: RwSignal<String>,
        #[prop(default = "text")] input_type: &'static str,
        #[prop(default = false)] required: bool,
        #[prop(default = "grid gap-1 text-sm")] class: &'static str,
    ) -> impl IntoView {
        let id = format!(
            "admin-{}",
            label
                .to_ascii_lowercase()
                .replace(|character: char| !character.is_ascii_alphanumeric(), "-")
        );
        let name = id.trim_start_matches("admin-").replace('-', "_");
        let input_id = id.clone();
        view! {
            <label class=class for=id.clone()>{label}
                <input
                    id=input_id
                    name=name
                    class="gw-input"
                    type=input_type
                    required=required
                    prop:value=move || value.get()
                    on:input=move |event| value.set(event_target_value(&event))
                />
            </label>
        }
    }

    #[component]
    fn ModalButtons(
        modal: RwSignal<Option<Modal>>,
        danger: bool,
        submit_label: &'static str,
        submit_icon: &'static str,
    ) -> impl IntoView {
        view! {
            <div class="flex justify-end gap-2">
                <button class="gw-btn" type="button" on:click=move |_| modal.set(None)>"Cancel"</button>
                <button class=if danger { "gw-btn gw-btn-danger" } else { "gw-btn gw-btn-primary" } type="submit">
                    <span class=format!("{submit_icon} h-4 w-4")></span>
                    {submit_label}
                </button>
            </div>
        }
    }

    fn refresh_data(
        config: AdminClientConfig,
        token: RwSignal<String>,
        limit: RwSignal<i64>,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        toast: RwSignal<Option<String>>,
    ) {
        spawn_local(async move {
            let token = token.get_untracked();
            let limit_value = limit.get_untracked();
            let overview_result =
                api_get::<OverviewResponse>("/api/overview", &config, &token).await;
            let jobs_result = api_get::<ListJobsResponse>(
                &format!("/api/jobs?state=all&limit={limit_value}"),
                &config,
                &token,
            )
            .await;
            match (overview_result, jobs_result) {
                (Ok(next_overview), Ok(next_jobs)) => {
                    let next_ids = next_jobs.jobs.iter().map(|job| job.id).collect::<Vec<_>>();
                    overview.set(next_overview);
                    jobs.set(next_jobs.jobs);
                    selected_jobs.update(|selected| selected.retain(|id| next_ids.contains(id)));
                }
                (Err(error), _) | (_, Err(error)) => show_toast(toast, error),
            }
        });
    }

    fn post_add_job(
        config: AdminClientConfig,
        token: RwSignal<String>,
        request: AddJobRequest,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) {
        spawn_local(async move {
            match api_post::<_, JobActionResponse>(
                "/api/jobs",
                &request,
                &config,
                &token.get_untracked(),
            )
            .await
            {
                Ok(response) => {
                    show_toast(toast, response.message);
                    modal.set(None);
                    refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
                }
                Err(error) => show_toast(toast, error),
            }
        });
    }

    fn post_job_action(
        config: AdminClientConfig,
        token: RwSignal<String>,
        request: JobActionRequest,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        toast: RwSignal<Option<String>>,
    ) {
        spawn_local(async move {
            match api_post::<_, JobActionResponse>(
                "/api/jobs/action",
                &request,
                &config,
                &token.get_untracked(),
            )
            .await
            {
                Ok(response) => {
                    show_toast(toast, response.message);
                    refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
                }
                Err(error) => show_toast(toast, error),
            }
        });
    }

    fn post_remove_key(
        config: AdminClientConfig,
        token: RwSignal<String>,
        request: RemoveJobByKeyRequest,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        modal: RwSignal<Option<Modal>>,
        toast: RwSignal<Option<String>>,
    ) {
        spawn_local(async move {
            match api_post::<_, MessageResponse>(
                "/api/jobs/remove-by-key",
                &request,
                &config,
                &token.get_untracked(),
            )
            .await
            {
                Ok(response) => {
                    show_toast(toast, response.message);
                    modal.set(None);
                    refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
                }
                Err(error) => show_toast(toast, error),
            }
        });
    }

    fn post_maintenance(
        config: AdminClientConfig,
        token: RwSignal<String>,
        request: MaintenanceRequest,
        overview: RwSignal<OverviewResponse>,
        jobs: RwSignal<Vec<ListedJob>>,
        selected_jobs: RwSignal<Vec<i64>>,
        limit: RwSignal<i64>,
        toast: RwSignal<Option<String>>,
    ) {
        spawn_local(async move {
            match api_post::<_, MessageResponse>(
                "/api/maintenance",
                &request,
                &config,
                &token.get_untracked(),
            )
            .await
            {
                Ok(response) => {
                    show_toast(toast, response.message);
                    refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
                }
                Err(error) => show_toast(toast, error),
            }
        });
    }

    async fn api_get<T>(path: &str, config: &AdminClientConfig, token: &str) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let response = with_api_headers(Request::get(&same_origin(path)), config, token, false)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        parse_response(response).await
    }

    async fn api_post<B, T>(
        path: &str,
        body: &B,
        config: &AdminClientConfig,
        token: &str,
    ) -> Result<T, String>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let request = with_api_headers(Request::post(&same_origin(path)), config, token, true)
            .json(body)
            .map_err(|error| error.to_string())?;
        let response = request.send().await.map_err(|error| error.to_string())?;
        parse_response(response).await
    }

    fn with_api_headers(
        builder: RequestBuilder,
        config: &AdminClientConfig,
        token: &str,
        writes: bool,
    ) -> RequestBuilder {
        let mut builder = builder
            .credentials(RequestCredentials::SameOrigin)
            .header("Accept", "application/json");

        if writes {
            builder = builder.header(&config.csrf_header, &config.csrf);
        }
        if config.auth_mode == "bearer" && !token.is_empty() {
            builder = builder.header("Authorization", &format!("Bearer {token}"));
        }
        if config.auth_mode == "header" && !token.is_empty() && !config.auth_header.is_empty() {
            builder = builder.header(&config.auth_header, token);
        }
        builder
    }

    async fn parse_response<T>(response: gloo_net::http::Response) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let status = response.status();
        let text = response.text().await.map_err(|error| error.to_string())?;
        if !(200..300).contains(&status) {
            return Err(serde_json::from_str::<ErrorResponse>(&text)
                .map(|error| error.error)
                .unwrap_or_else(|_| format!("{status}: {text}")));
        }
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }

    fn same_origin(path: &str) -> String {
        let Some(window) = web_sys::window() else {
            return path.to_string();
        };
        let location = window.location();
        let protocol = location.protocol().unwrap_or_default();
        let host = location.host().unwrap_or_default();
        format!("{protocol}//{host}{path}")
    }

    fn apply_theme(theme: &str, accent: &str, compact: bool) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(root) = document.document_element() else {
            return;
        };
        let prefers_dark = window
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .is_some_and(|query| query.matches());
        let class_list = root.class_list();
        let _ = class_list.toggle_with_force(
            "dark",
            theme == "dark" || (theme == "system" && prefers_dark),
        );
        let _ = class_list.toggle_with_force("theme-emerald", accent == "emerald");
        let _ = class_list.toggle_with_force("theme-violet", accent == "violet");
        let _ = class_list.toggle_with_force("theme-amber", accent == "amber");
        if let Some(body) = document.body() {
            let _ = body.class_list().toggle_with_force("gw-compact", compact);
        }
    }

    fn copy_to_clipboard(
        text: String,
        message: impl Into<String>,
        toast: RwSignal<Option<String>>,
    ) {
        let message = message.into();
        let Some(window) = web_sys::window() else {
            show_toast(toast, "Clipboard is unavailable");
            return;
        };
        let clipboard = window.navigator().clipboard();
        spawn_local(async move {
            match JsFuture::from(clipboard.write_text(&text)).await {
                Ok(_) => show_toast(toast, message),
                Err(_) => show_toast(toast, "Clipboard write failed"),
            }
        });
    }

    fn show_toast(toast: RwSignal<Option<String>>, message: impl Into<String>) {
        toast.set(Some(message.into()));
        Timeout::new(2600, move || toast.set(None)).forget();
    }

    fn storage_get(key: &str) -> Option<String> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .and_then(|storage| storage.get_item(key).ok().flatten())
    }

    fn storage_set(key: &str, value: &str) {
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            let _ = storage.set_item(key, value);
        }
    }

    fn storage_remove(key: &str) {
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            let _ = storage.remove_item(key);
        }
    }

    fn filter_values(raw: &str) -> Vec<String> {
        raw.split([',', '\n', '\r'])
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_lowercase)
            .collect()
    }

    fn normalize_filter_paste(raw: &str) -> String {
        raw.split([',', '\n', '\r', '\t'])
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn matches_column(job: &ListedJob, column: &str, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        let value = match column {
            "task_identifier" => job.task_identifier.clone(),
            "queue_name" => job.queue_name.clone().unwrap_or_default(),
            "key" => job.key.clone().unwrap_or_default(),
            "locked_by" => job.locked_by.clone().unwrap_or_default(),
            _ => String::new(),
        }
        .to_lowercase();
        filters.iter().any(|filter| value.contains(filter))
    }

    fn job_search_text(job: &ListedJob) -> String {
        [
            job.id.to_string(),
            job.task_identifier.clone(),
            job.queue_name.clone().unwrap_or_default(),
            job.key.clone().unwrap_or_default(),
            job.locked_by.clone().unwrap_or_default(),
            job.last_error.clone().unwrap_or_default(),
            stringify_value(&job.payload),
        ]
        .join(" ")
        .to_lowercase()
    }

    fn job_state(job: &ListedJob) -> JobState {
        if job.locked_at.is_some() {
            return JobState::Locked;
        }
        if job.attempts >= job.max_attempts {
            return JobState::Failed;
        }
        if job.run_at > Utc::now() {
            return JobState::Scheduled;
        }
        JobState::Ready
    }

    fn state_label(state: JobState) -> &'static str {
        match state {
            JobState::All => "all",
            JobState::Ready => "ready",
            JobState::Scheduled => "scheduled",
            JobState::Locked => "locked",
            JobState::Failed => "failed",
        }
    }

    fn state_color(state: JobState) -> &'static str {
        match state {
            JobState::Ready => "text-emerald-600 dark:text-emerald-300",
            JobState::Scheduled => "text-amber-600 dark:text-amber-300",
            JobState::Locked => "text-cyan-600 dark:text-cyan-300",
            JobState::Failed => "text-rose-600 dark:text-rose-300",
            JobState::All => "",
        }
    }

    fn selected_rows(
        jobs: RwSignal<Vec<ListedJob>>,
        selected: RwSignal<Vec<i64>>,
    ) -> Vec<ListedJob> {
        let selected = selected.get_untracked();
        jobs.get_untracked()
            .into_iter()
            .filter(|job| selected.contains(&job.id))
            .collect()
    }

    fn selected_csv(rows: &[ListedJob]) -> String {
        let columns = [
            "id",
            "task_identifier",
            "queue_name",
            "state",
            "run_at",
            "attempts",
            "max_attempts",
            "priority",
            "key",
        ];
        let mut output = vec![columns.join(",")];
        for job in rows {
            output.push(
                [
                    job.id.to_string(),
                    job.task_identifier.clone(),
                    job.queue_name.clone().unwrap_or_default(),
                    state_label(job_state(job)).to_string(),
                    job.run_at.to_rfc3339(),
                    job.attempts.to_string(),
                    job.max_attempts.to_string(),
                    job.priority.to_string(),
                    job.key.clone().unwrap_or_default(),
                ]
                .into_iter()
                .map(csv_escape)
                .collect::<Vec<_>>()
                .join(","),
            );
        }
        output.join("\n")
    }

    fn csv_escape(value: String) -> String {
        format!("\"{}\"", value.replace('"', "\"\""))
    }

    fn stringify_value(value: &Value) -> String {
        serde_json::to_string(value).unwrap_or_default()
    }

    fn short(value: &str, length: usize) -> String {
        if value.chars().count() <= length {
            return value.to_string();
        }
        let mut output = value
            .chars()
            .take(length.saturating_sub(3))
            .collect::<String>();
        output.push_str("...");
        output
    }

    fn format_date(value: DateTime<Utc>) -> String {
        value.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    fn textarea_value(event: &Event) -> String {
        event
            .target()
            .and_then(|target| target.dyn_into::<HtmlTextAreaElement>().ok())
            .map(|target| target.value())
            .unwrap_or_default()
    }

    fn optional_string(value: String) -> Option<String> {
        let value = value.trim().to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    fn optional_i16(value: String) -> Option<i16> {
        value.trim().parse().ok()
    }

    fn optional_csv(value: String) -> Option<Vec<String>> {
        let values = value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if values.is_empty() {
            None
        } else {
            Some(values)
        }
    }

    fn datetime_local_to_utc(value: &str) -> Option<DateTime<Utc>> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        let format = if value.matches(':').count() >= 2 {
            "%Y-%m-%dT%H:%M:%S"
        } else {
            "%Y-%m-%dT%H:%M"
        };
        NaiveDateTime::parse_from_str(value, format)
            .ok()
            .map(|datetime| {
                let js_local = js_sys::Date::new(&JsValue::from_str(
                    &datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                ));
                let offset_minutes = js_local.get_timezone_offset() as i64;
                DateTime::<Utc>::from_naive_utc_and_offset(
                    datetime + Duration::minutes(offset_minutes),
                    Utc,
                )
            })
    }

    fn modal_title(modal: &Option<Modal>) -> &'static str {
        match modal {
            Some(Modal::AddJob) => "Add job",
            Some(Modal::FailJobs) => "Fail selected jobs",
            Some(Modal::Reschedule) => "Reschedule selected jobs",
            Some(Modal::RemoveKey) => "Remove job by key",
            Some(Modal::JobDetails(_)) => "Job details",
            None => "",
        }
    }
}
