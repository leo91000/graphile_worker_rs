use leptos::prelude::*;

use super::super::api::{refresh_data, RefreshSignals};
use super::super::browser::{storage_remove, storage_set};
use super::super::types::AdminClientConfig;

#[component]
pub(super) fn AuthTokenPanel(
    show_token_login: bool,
    config: AdminClientConfig,
    refresh: RefreshSignals,
) -> impl IntoView {
    view! {
    <section class=if show_token_login { "gw-panel mb-4 p-4" } else { "hidden" }>
        <div class="flex flex-wrap items-end gap-3">
            <div class="min-w-72 flex-1">
                <label class="mb-1 block text-sm font-medium" for="auth-token">"API token"</label>
                <input
                    id="auth-token"
                    class="gw-input w-full"
                    type="password"
                    autocomplete="current-password"
                    prop:value=move || refresh.token.get()
                    on:input=move |event| refresh.token.set(event_target_value(&event))
                />
            </div>
            <button
                class="gw-btn gw-btn-primary"
                type="button"
                on:click={
                    let config = config.clone();
                    move |_| {
                        storage_set("gw-admin-token", &refresh.token.get_untracked());
                        refresh_data(config.clone(), refresh);
                    }
                }
            >
                <span class="i-lucide-key-round h-4 w-4"></span>
                "Use token"
            </button>
            <button class="gw-btn" type="button" on:click=move |_| {
                storage_remove("gw-admin-token");
                refresh.token.set(String::new());
            }>"Clear"</button>
        </div>
    </section>
        }
}
