use std::fmt;

use crate::limits::Limits;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("name must not be empty")]
    Empty,
    #[error("name must be at most {max} characters")]
    TooLong { max: usize },
}

/// A validated rack, device or port name: no whitespace, non-empty,
/// at most `Limits::name_max_len` characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(String);

impl Name {
    pub fn parse(input: &str, limits: &Limits) -> Result<Name, NameError> {
        let normalized: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if normalized.is_empty() {
            return Err(NameError::Empty);
        }
        if normalized.chars().count() > limits.name_max_len {
            return Err(NameError::TooLong { max: limits.name_max_len });
        }
        Ok(Name(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The form used for uniqueness checks (names are case-insensitive).
    pub fn key(&self) -> String {
        self.0.to_lowercase()
    }

    pub fn same_as(&self, other: &Name) -> bool {
        self.key() == other.key()
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Returns `base` if it is free, otherwise the first free `base_N` (N ≥ 2),
/// truncating `base` so the result fits `limits.name_max_len`.
pub fn auto_rename(base: &Name, is_taken: impl Fn(&Name) -> bool, limits: &Limits) -> Name {
    if !is_taken(base) {
        return base.clone();
    }
    for n in 2u32.. {
        let suffix = format!("_{n}");
        let keep = limits.name_max_len.saturating_sub(suffix.chars().count());
        let truncated: String = base.0.chars().take(keep).collect();
        let candidate = Name(format!("{truncated}{suffix}"));
        if !is_taken(&candidate) {
            return candidate;
        }
    }
    unreachable!("the suffix range is unbounded")
}

pub const DOCUMENT_NAME_MAX_LEN: usize = 100;

const FORBIDDEN_DOCUMENT_CHARS: &str = "<>:\"/\\|?*";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentNameError {
    #[error("document name must not be empty")]
    Empty,
    #[error("document name must be at most {max} characters")]
    TooLong { max: usize },
    #[error("document name must not contain '{0}'")]
    ForbiddenChar(char),
    #[error("document name must not contain control characters")]
    ControlChar,
    #[error("document name must not end with '.'")]
    TrailingDot,
    #[error("'{0}' is a reserved name")]
    Reserved(String),
}

/// A document name that is safe to use as a Windows file name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentName(String);

impl DocumentName {
    pub fn parse(input: &str) -> Result<DocumentName, DocumentNameError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(DocumentNameError::Empty);
        }
        if trimmed.chars().count() > DOCUMENT_NAME_MAX_LEN {
            return Err(DocumentNameError::TooLong { max: DOCUMENT_NAME_MAX_LEN });
        }
        for c in trimmed.chars() {
            if c.is_control() {
                return Err(DocumentNameError::ControlChar);
            }
            if FORBIDDEN_DOCUMENT_CHARS.contains(c) {
                return Err(DocumentNameError::ForbiddenChar(c));
            }
        }
        if trimmed.ends_with('.') {
            return Err(DocumentNameError::TrailingDot);
        }
        if is_reserved_windows_name(trimmed) {
            return Err(DocumentNameError::Reserved(trimmed.to_string()));
        }
        Ok(DocumentName(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocumentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_reserved_windows_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let bytes = upper.as_bytes();
    bytes.len() == 4
        && (upper.starts_with("COM") || upper.starts_with("LPT"))
        && (b'1'..=b'9').contains(&bytes[3])
}
