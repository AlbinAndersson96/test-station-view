//! Toolbar, modal dialogs and file download/upload (spec §4.3 "Toolbar", "Dialogs"; §5).

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::ui::core::{Dialog, now_s, read, save_now, signals, update};
use crate::ui::icons::{self, Icon};

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
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let anchor = document
        .create_element("a")
        .ok()
        .and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok());
    if let (Some(a), Some(body)) = (anchor, document.body()) {
        // Some browsers only download from an anchor that is in the document.
        a.set_href(&url);
        a.set_download(file_name);
        let _ = body.append_child(&a);
        a.click();
        a.remove();
    }
    // Revoke later: revoking immediately can cancel the download in some browsers.
    let revoke = wasm_bindgen::prelude::Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000);
}

/// The project's source repository, linked from the toolbar.
const REPO_URL: &str = "https://github.com/AlbinAndersson96/test-station-view";

/// The GitHub mark (Octicons `mark-github`, 16×16).
const GITHUB_MARK: &str = "M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z";

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
            <span class="brand">
                <span class="brand-mark"><Icon d=icons::LOGO /></span>
                <span class="app-name">"TestStationView"</span>
            </span>
            <div class="tool-group">
                <button class="ghost" title="New document" on:click=move |_| sig.dialog.set(Some(Dialog::ConfirmNew))>
                    <Icon d=icons::NEW />"New"
                </button>
                <button class="ghost" title="Import a document from a file" on:click=move |_| {
                    if let Some(input) = file_ref.get() {
                        input.click();
                    }
                }>
                    <Icon d=icons::IMPORT />"Import"
                </button>
                <input node_ref=file_ref type="file" accept=".json,application/json" hidden on:change=on_file />
                <button class="ghost" title="Download the document as JSON" on:click=move |_| {
                    let (name, json) = read(|s| s.export());
                    download(&name, &json);
                }>
                    <Icon d=icons::EXPORT />"Export"
                </button>
            </div>
            <span class="separator"></span>
            <div class="tool-group">
                <button class="ghost icon-only" title="Undo (Ctrl+Z)" aria-label="Undo"
                    prop:disabled=move || !can_undo() on:click=move |_| update(|s| s.undo())>
                    <Icon d=icons::UNDO />
                </button>
                <button class="ghost icon-only" title="Redo (Ctrl+Y)" aria-label="Redo"
                    prop:disabled=move || !can_redo() on:click=move |_| update(|s| s.redo())>
                    <Icon d=icons::REDO />
                </button>
            </div>
            <span class="separator"></span>
            <button class="ghost" title="Return to the front view of all racks" on:click=move |_| update(|s| s.reset_view(now_s()))>
                <Icon d=icons::RESET_VIEW />"Reset view"
            </button>
            <a class="repo-link" href=REPO_URL target="_blank" rel="noopener noreferrer" title="Source on GitHub">
                <svg class="icon" viewBox="0 0 16 16" aria-hidden="true">
                    <path fill="currentColor" stroke="none" d=GITHUB_MARK />
                </svg>
                "GitHub"
            </a>
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
                    <p class="modal-message">{message}</p>
                    <div class="modal-actions">{actions}</div>
                </div>
            </div>
        })
    }
}
