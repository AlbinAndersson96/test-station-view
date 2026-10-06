//! Inline line icons (24×24, stroked with the current text colour).

use leptos::prelude::*;

pub const LOGO: &str = "M5 3h14a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z M5 13h14a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z M7 7h.01 M7 17h.01 M11 7h6 M11 17h6";
pub const NEW: &str =
    "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M12 18v-6 M9 15h6";
pub const IMPORT: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M17 8l-5-5-5 5 M12 3v12";
pub const EXPORT: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M7 10l5 5 5-5 M12 15V3";
pub const UNDO: &str = "M9 14L4 9l5-5 M4 9h10.5a5.5 5.5 0 0 1 0 11H11";
pub const REDO: &str = "M15 14l5-5-5-5 M20 9H9.5a5.5 5.5 0 0 0 0 11H13";
pub const RESET_VIEW: &str = "M3 7V5a2 2 0 0 1 2-2h2 M17 3h2a2 2 0 0 1 2 2v2 M21 17v2a2 2 0 0 1-2 2h-2 M7 21H5a2 2 0 0 1-2-2v-2 M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6";
pub const PLUS: &str = "M12 5v14 M5 12h14";
pub const CHEVRON: &str = "M9 18l6-6-6-6";
pub const DOCUMENT: &str = "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6";
pub const RACK: &str = "M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z M4 8h16 M4 14h16 M8 5h.01 M8 11h.01 M8 17h.01";
pub const PORT: &str = "M5 5h14v14H5z M9 10h6v4H9z";
pub const GRIP: &str = "M9 6h.01 M9 12h.01 M9 18h.01 M15 6h.01 M15 12h.01 M15 18h.01";
pub const TRASH: &str = "M3 6h18 M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6 M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2 M10 11v6 M14 11v6";
pub const RENAME: &str = "M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z";
pub const HELP: &str =
    "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20 M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3 M12 17h.01";
pub const WARNING: &str = "M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z M12 9v4 M12 17h.01";

/// One icon; `d` is one of the path constants above.
#[component]
pub fn Icon(d: &'static str) -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d=d />
        </svg>
    }
}
