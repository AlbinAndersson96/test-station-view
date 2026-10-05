//! Toolbar, modal dialogs and file download/upload (spec §4.3 "Toolbar", "Dialogs"; §5).

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::ui::core::{Dialog, now_s, read, save_now, signals, update};

/// Offers `text` to the user as a file download named `file_name`.
pub fn download(file_name: &str, text: &str) {
    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let anchor = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("a").ok())
        .and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok());
    if let Some(a) = anchor {
        a.set_href(&url);
        a.set_download(file_name);
        a.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

#[component]
pub fn Toolbar() -> impl IntoView {
    let sig = signals();
    let file_ref = NodeRef::<leptos::html::Input>::new();
    let on_file = move |_| {
        let Some(input) = file_ref.get() else { return };
        let Some(file) = input.files().and_then(|f| f.get(0)) else {
            return;
        };
        input.set_value("");
        leptos::task::spawn_local(async move {
            match JsFuture::from(file.text()).await {
                Ok(text) => {
                    let text = text.as_string().unwrap_or_default();
                    signals().dialog.set(Some(Dialog::ConfirmImport { text }));
                }
                Err(_) => signals().dialog.set(Some(Dialog::Error {
                    message: "The file could not be read.".into(),
                })),
            }
        });
    };
    let can_undo = move || {
        sig.rev.track();
        read(|s| s.can_undo())
    };
    let can_redo = move || {
        sig.rev.track();
        read(|s| s.can_redo())
    };
    view! {
        <header class="toolbar">
            <span class="app-name">"TestStationView"</span>
            <button on:click=move |_| sig.dialog.set(Some(Dialog::ConfirmNew))>"New"</button>
            <button on:click=move |_| {
                if let Some(input) = file_ref.get() {
                    input.click();
                }
            }>"Import"</button>
            <input node_ref=file_ref type="file" accept=".json,application/json" hidden on:change=on_file />
            <button on:click=move |_| {
                let (name, json) = read(|s| s.export());
                download(&name, &json);
            }>"Export"</button>
            <span class="separator"></span>
            <button prop:disabled=move || !can_undo() on:click=move |_| update(|s| s.undo())>"Undo"</button>
            <button prop:disabled=move || !can_redo() on:click=move |_| update(|s| s.redo())>"Redo"</button>
            <span class="separator"></span>
            <button on:click=move |_| update(|s| s.reset_view(now_s()))>"Reset view"</button>
        </header>
    }
}

#[component]
pub fn DialogHost() -> impl IntoView {
    let sig = signals();
    let close = move || sig.dialog.set(None);
    move || {
        let dialog = sig.dialog.get()?;
        let (title, message, actions): (&str, String, AnyView) = match dialog {
            Dialog::ConfirmNew => (
                "New document",
                "Replace the current document with a new one? You can undo this.".into(),
                view! {
                    <button on:click=move |_| close()>"Cancel"</button>
                    <button class="primary" on:click=move |_| {
                        close();
                        update(|s| s.new_document(now_s()));
                    }>"Create new"</button>
                }
                    .into_any(),
            ),
            Dialog::ConfirmImport { text } => (
                "Import",
                "Replace the current document with the imported file? Unsaved changes will be lost; Export first to keep them.".into(),
                view! {
                    <button on:click=move |_| close()>"Cancel"</button>
                    <button class="primary" on:click=move |_| {
                        let result = update(|s| s.import(&text, now_s()));
                        match result {
                            Ok(()) => close(),
                            Err(message) => sig.dialog.set(Some(Dialog::Error { message })),
                        }
                    }>"Import"</button>
                }
                    .into_any(),
            ),
            Dialog::ConfirmDeleteRack { rack, name } => (
                "Delete rack",
                format!("Delete rack '{name}' and all its equipment?"),
                view! {
                    <button on:click=move |_| close()>"Cancel"</button>
                    <button class="primary danger" on:click=move |_| {
                        close();
                        update(|s| s.delete(tsv_core::edit::ObjectId::Rack(rack)));
                    }>"Delete"</button>
                }
                    .into_any(),
            ),
            Dialog::Error { message } => (
                "Something went wrong",
                message,
                view! { <button class="primary" on:click=move |_| close()>"OK"</button> }.into_any(),
            ),
            Dialog::StartupProblem => {
                let reason = read(|s| s.startup_problem().map(|p| p.reason.clone())).unwrap_or_default();
                (
                    "Saved document could not be loaded",
                    format!("{reason} Download the stored data to keep it, or start a new document."),
                    view! {
                        <button on:click=move |_| {
                            if let Some(raw) = read(|s| s.startup_problem().map(|p| p.raw.clone())) {
                                download("teststationview-recovered.json", &raw);
                            }
                        }>"Download the stored data"</button>
                        <button class="primary" on:click=move |_| {
                            close();
                            update(|s| s.set_startup_problem(None));
                            save_now();
                        }>"Start a new document"</button>
                    }
                        .into_any(),
                )
            }
            Dialog::DeviceLost => (
                "The graphics device was lost",
                "Reload the page to continue. Your document is saved in this browser.".into(),
                view! {
                    <button class="primary" on:click=move |_| {
                        if let Some(w) = web_sys::window() {
                            let _ = w.location().reload();
                        }
                    }>"Reload"</button>
                }
                    .into_any(),
            ),
        };
        Some(view! {
            <div class="modal-backdrop">
                <div class="modal" role="dialog" aria-modal="true">
                    <h2>{title}</h2>
                    <p>{message}</p>
                    <div class="modal-actions">{actions}</div>
                </div>
            </div>
        })
    }
}
