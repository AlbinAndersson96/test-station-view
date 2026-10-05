//! Persistence (spec §5): a swappable store of the document's JSON, and startup recovery.

use std::cell::{Cell, RefCell};
use std::future::Future;

use tsv_core::file_format::from_json;
use tsv_core::limits::Limits;
use tsv_core::model::Document;

use crate::session::StartupProblem;

/// Browser local-storage key of the autosaved document.
pub const STORAGE_KEY: &str = "teststationview.document";

pub const SAVE_FAILED_BANNER: &str =
    "Changes can't be saved in this browser — use Export to keep your work.";

/// Where the current document is kept between sessions. Async so a server-backed store can
/// implement it later without changing callers.
pub trait DocumentStore {
    /// The stored document's JSON, if any.
    fn load(&self) -> impl Future<Output = Result<Option<String>, String>>;
    fn save(&self, json: String) -> impl Future<Output = Result<(), String>>;
}

/// What to start with, given what the store returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Startup {
    /// Nothing stored (or storage unavailable): the default document.
    Fresh(Document),
    Restored(Document),
    /// Stored data exists but cannot be loaded. Start from the default document, show the
    /// problem, and do not overwrite the stored data until the user decides.
    Unreadable {
        fallback: Document,
        problem: StartupProblem,
    },
}

pub fn interpret_stored(stored: Result<Option<String>, String>, limits: &Limits) -> Startup {
    match stored {
        Ok(Some(raw)) => match from_json(&raw, limits) {
            Ok(doc) => Startup::Restored(doc),
            Err(e) => Startup::Unreadable {
                fallback: Document::new_default(limits),
                problem: StartupProblem {
                    raw,
                    reason: e.to_string(),
                },
            },
        },
        Ok(None) | Err(_) => Startup::Fresh(Document::new_default(limits)),
    }
}

/// In-memory store for tests.
#[derive(Debug, Default)]
pub struct MemoryStore {
    pub data: RefCell<Option<String>>,
    /// When set, `save` fails like a full or disabled browser storage.
    pub fail_saves: Cell<bool>,
}

impl DocumentStore for MemoryStore {
    fn load(&self) -> impl Future<Output = Result<Option<String>, String>> {
        std::future::ready(Ok(self.data.borrow().clone()))
    }

    fn save(&self, json: String) -> impl Future<Output = Result<(), String>> {
        let result = if self.fail_saves.get() {
            Err("storage unavailable".to_string())
        } else {
            *self.data.borrow_mut() = Some(json);
            Ok(())
        };
        std::future::ready(result)
    }
}

/// The browser's `localStorage`.
#[cfg(target_arch = "wasm32")]
pub struct LocalStorageStore;

#[cfg(target_arch = "wasm32")]
impl LocalStorageStore {
    fn storage() -> Result<web_sys::Storage, String> {
        web_sys::window()
            .ok_or("no window")?
            .local_storage()
            .map_err(|_| "local storage is blocked".to_string())?
            .ok_or_else(|| "local storage is unavailable".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
impl DocumentStore for LocalStorageStore {
    fn load(&self) -> impl Future<Output = Result<Option<String>, String>> {
        let result = Self::storage().and_then(|s| {
            s.get_item(STORAGE_KEY)
                .map_err(|_| "could not read local storage".to_string())
        });
        std::future::ready(result)
    }

    fn save(&self, json: String) -> impl Future<Output = Result<(), String>> {
        let result = Self::storage().and_then(|s| {
            s.set_item(STORAGE_KEY, &json)
                .map_err(|_| "could not write local storage".to_string())
        });
        std::future::ready(result)
    }
}

/// Saves the session's document if autosave is allowed, and updates the banner.
pub async fn autosave(session: &RefCell<crate::session::Session>, store: &impl DocumentStore) {
    let payload = {
        let s = session.borrow();
        if !s.autosave_allowed() {
            return;
        }
        s.save_payload()
    };
    let result = store.save(payload).await;
    let banner = result.err().map(|_| SAVE_FAILED_BANNER.to_string());
    session.borrow_mut().set_banner(banner);
}
