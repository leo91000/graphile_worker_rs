use leptos::prelude::*;

#[component]
pub(super) fn QueuesWorkersPanel() -> impl IntoView {
    view! {
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
    }
}
