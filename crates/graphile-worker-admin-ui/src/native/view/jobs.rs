use leptos::prelude::*;

#[component]
pub(super) fn JobsPanel() -> impl IntoView {
    view! {
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
    }
}
