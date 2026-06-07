use leptos::prelude::*;

use super::super::types::Modal;

#[component]
pub(super) fn TextInput(
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
pub(super) fn ModalButtons(
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
