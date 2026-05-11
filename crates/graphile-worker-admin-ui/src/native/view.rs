use leptos::prelude::*;

use super::auth::{AdminAuthSummary, CSRF_HEADER};

#[derive(Debug)]
pub struct AdminUiRenderConfig {
    pub csrf_token: String,
    pub schema: String,
    pub read_only: bool,
    pub auth: AdminAuthSummary,
}

pub fn render_admin_html(config: &AdminUiRenderConfig) -> String {
    let auth_mode = config.auth.mode.as_str();
    let header_name = config.auth.header_name.as_deref().unwrap_or("");
    let read_only = config.read_only.to_string();
    let app = view! {
        <div
            id="gw-admin"
            class="gw-shell"
            data-auth-mode=auth_mode
            data-auth-header=header_name
            data-read-only=read_only
            data-csrf=config.csrf_token.clone()
            data-csrf-header=CSRF_HEADER
            data-schema=config.schema.clone()
        >
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
                        <span class="font-mono">{config.schema.clone()}</span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="gw-muted">"Auth"</span>
                        <span id="auth-mode-label" class="gw-pill">{auth_mode}</span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="gw-muted">"Writes"</span>
                        <span class="gw-pill">{if config.read_only { "read only" } else { "enabled" }}</span>
                    </div>
                </div>
            </aside>

            <main class="gw-main">
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

                <div class="gw-scroll">
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

                    <section id="overview" class="grid gap-3 md:grid-cols-5">
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Total"</span>
                                <span class="i-lucide-database h-4 w-4 gw-muted"></span>
                            </div>
                            <strong id="stat-total" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Ready"</span>
                                <span class="i-lucide-play h-4 w-4 text-emerald-500"></span>
                            </div>
                            <strong id="stat-ready" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Scheduled"</span>
                                <span class="i-lucide-clock h-4 w-4 text-amber-500"></span>
                            </div>
                            <strong id="stat-scheduled" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Locked"</span>
                                <span class="i-lucide-lock h-4 w-4 text-cyan-500"></span>
                            </div>
                            <strong id="stat-locked" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Failed"</span>
                                <span class="i-lucide-circle-alert h-4 w-4 text-rose-500"></span>
                            </div>
                            <strong id="stat-failed" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                    </section>

                    <section id="jobs" class="gw-panel mt-4">
                        <div class="flex flex-wrap items-center justify-between gap-3 border-b p-3" style="border-color: rgb(var(--border));">
                            <div class="flex flex-wrap items-center gap-2" role="tablist" aria-label="Job state">
                                <button class="gw-tab job-state-tab" data-state="all" aria-selected="true" type="button">"All"</button>
                                <button class="gw-tab job-state-tab" data-state="ready" type="button">"Ready"</button>
                                <button class="gw-tab job-state-tab" data-state="scheduled" type="button">"Scheduled"</button>
                                <button class="gw-tab job-state-tab" data-state="locked" type="button">"Locked"</button>
                                <button class="gw-tab job-state-tab" data-state="failed" type="button">"Failed"</button>
                            </div>
                            <div class="flex flex-wrap items-center gap-2">
                                <button id="add-job-btn" class="gw-btn gw-btn-primary" type="button">
                                    <span class="i-lucide-plus h-4 w-4"></span>
                                    "Add"
                                </button>
                                <button id="copy-selected-json-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-copy h-4 w-4"></span>
                                    "JSON"
                                </button>
                                <button id="copy-selected-csv-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-clipboard h-4 w-4"></span>
                                    "CSV"
                                </button>
                            </div>
                        </div>

                        <div class="grid gap-2 border-b p-3 lg:grid-cols-[minmax(240px,1fr)_repeat(4,minmax(120px,180px))_110px]" style="border-color: rgb(var(--border));">
                            <input id="global-search" class="gw-input" type="search" placeholder="Search id, task, queue, key, worker, payload..." />
                            <input class="gw-input column-filter" name="task_filter" data-column="task_identifier" type="search" placeholder="Task filter" />
                            <input class="gw-input column-filter" name="queue_filter" data-column="queue_name" type="search" placeholder="Queue filter" />
                            <input class="gw-input column-filter" name="key_filter" data-column="key" type="search" placeholder="Key filter" />
                            <input class="gw-input column-filter" name="worker_filter" data-column="locked_by" type="search" placeholder="Worker filter" />
                            <select id="limit-select" class="gw-input">
                                <option value="50">"50"</option>
                                <option value="100" selected>"100"</option>
                                <option value="250">"250"</option>
                                <option value="500">"500"</option>
                            </select>
                        </div>

                        <div class="flex flex-wrap items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                            <span id="selection-count" class="gw-pill">"0 selected"</span>
                            <button id="complete-selected-btn" class="gw-btn" type="button">
                                <span class="i-lucide-check h-4 w-4"></span>
                                "Complete"
                            </button>
                            <button id="run-now-selected-btn" class="gw-btn" type="button">
                                <span class="i-lucide-play h-4 w-4"></span>
                                "Run now"
                            </button>
                            <button id="reschedule-selected-btn" class="gw-btn" type="button">
                                <span class="i-lucide-calendar-clock h-4 w-4"></span>
                                "Reschedule"
                            </button>
                            <button id="fail-selected-btn" class="gw-btn gw-btn-danger" type="button">
                                <span class="i-lucide-ban h-4 w-4"></span>
                                "Fail"
                            </button>
                            <button id="remove-key-btn" class="gw-btn" type="button">
                                <span class="i-lucide-key-x h-4 w-4"></span>
                                "Remove by key"
                            </button>
                        </div>

                        <div class="max-h-[58vh] overflow-auto">
                            <table class="gw-table">
                                <thead>
                                    <tr>
                                        <th><input id="select-all-jobs" type="checkbox" class="h-4 w-4" /></th>
                                        <th>"ID"</th>
                                        <th>"Task"</th>
                                        <th>"Queue"</th>
                                        <th>"State"</th>
                                        <th>"Run at"</th>
                                        <th>"Attempts"</th>
                                        <th>"Priority"</th>
                                        <th>"Key"</th>
                                        <th>"Payload"</th>
                                        <th>"Error"</th>
                                        <th>"Actions"</th>
                                    </tr>
                                </thead>
                                <tbody id="jobs-tbody">
                                    <tr>
                                        <td colspan="12" class="py-8 text-center gw-muted">"Loading jobs..."</td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </section>

                    <section id="queues-workers" class="mt-4 grid gap-4 lg:grid-cols-2">
                        <div id="queues" class="gw-panel">
                            <div class="flex items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                                <span class="i-lucide-git-branch h-4 w-4"></span>
                                <h3 class="font-semibold">"Queues"</h3>
                            </div>
                            <div id="queues-list" class="divide-y" style="border-color: rgb(var(--border));"></div>
                        </div>
                        <div id="workers" class="gw-panel">
                            <div class="flex items-center justify-between gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                                <div class="flex items-center gap-2">
                                    <span class="i-lucide-hard-drive h-4 w-4"></span>
                                    <h3 class="font-semibold">"Workers"</h3>
                                </div>
                                <button id="force-unlock-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-unlock h-4 w-4"></span>
                                    "Force unlock"
                                </button>
                            </div>
                            <div id="workers-list" class="divide-y" style="border-color: rgb(var(--border));"></div>
                        </div>
                    </section>

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
                </div>
            </main>

            <div id="modal" class="gw-modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
                <div class="gw-dialog">
                    <div class="mb-4 flex items-center justify-between gap-3">
                        <h3 id="modal-title" class="text-lg font-semibold"></h3>
                        <button id="modal-close" class="gw-btn" type="button" aria-label="Close">
                            <span class="i-lucide-x h-4 w-4"></span>
                        </button>
                    </div>
                    <div id="modal-body"></div>
                </div>
            </div>

            <div id="toast" class="gw-toast"></div>
        </div>
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <title>Graphile Worker Admin</title>
  <link rel="icon" href="/favicon.ico" type="image/svg+xml">
  <link rel="stylesheet" href="/assets/admin.css">
</head>
<body>
{body}
<script type="module" src="/assets/admin.js"></script>
</body>
</html>"#,
        body = app.to_html()
    )
}
