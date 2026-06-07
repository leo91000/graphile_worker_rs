use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use leptos::prelude::*;

use super::super::api::{refresh_data, RefreshSignals};
use super::super::browser::storage_set;
use super::super::types::AdminClientConfig;

#[component]
pub(super) fn Topbar(
    config: AdminClientConfig,
    refresh: RefreshSignals,
    theme: RwSignal<String>,
    accent: RwSignal<String>,
    compact: RwSignal<bool>,
    auto_refresh_enabled: RwSignal<bool>,
    auto_refresh_timer: Rc<RefCell<Option<Interval>>>,
) -> impl IntoView {
    let refresh_click = {
        let config = config.clone();
        move |_| {
            refresh_data(config.clone(), refresh);
        }
    };

    view! {
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
                    prop:checked=move || auto_refresh_enabled.get()
                    on:change={
                        let config = config.clone();
                        let auto_refresh_timer = auto_refresh_timer.clone();
                        move |event| {
                            if event_target_checked(&event) {
                                auto_refresh_enabled.set(true);
                                if auto_refresh_timer.borrow().is_some() {
                                    return;
                                }
                                let config = config.clone();
                                let handle = Interval::new(5000, move || {
                                    refresh_data(config.clone(), refresh);
                                });
                                *auto_refresh_timer.borrow_mut() = Some(handle);
                            } else {
                                auto_refresh_enabled.set(false);
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
        }
}
