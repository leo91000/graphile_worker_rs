use gloo_timers::callback::Timeout;
use leptos::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

pub(super) fn apply_theme(theme: &str, accent: &str, compact: bool) {
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

pub(super) fn copy_to_clipboard(
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

pub(super) fn show_toast(toast: RwSignal<Option<String>>, message: impl Into<String>) {
    toast.set(Some(message.into()));
    Timeout::new(2600, move || toast.set(None)).forget();
}

pub(super) fn storage_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

pub(super) fn storage_set(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

pub(super) fn storage_remove(key: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.remove_item(key);
    }
}
