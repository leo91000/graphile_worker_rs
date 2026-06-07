use leptos::prelude::*;

#[component]
pub(super) fn AdminModal() -> impl IntoView {
    view! {
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
    }
}
