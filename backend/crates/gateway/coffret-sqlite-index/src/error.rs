use std::error;
use std::fmt;
use std::path::Path;

use coffret_usecase::{IndexError, IndexResult};
use rusqlite::ffi;

/// Turns a SQLite failure into the port's vocabulary.
///
/// The whole point of doing it here is that nothing above this crate sees a
/// SQLite error code, and nothing above it reads a message to find out what
/// happened. The failure travels as the structured cause of
/// [`IndexError::Backend`] rather than flattened into a string, so a caller that
/// wants the detail can still reach it through `source`.
///
/// What the Index was doing comes in alongside, because a bare "database is
/// locked" says nothing about which of a catalog's operations hit it.
pub(crate) fn translate(operation: &'static str) -> impl Fn(rusqlite::Error) -> IndexError {
    move |error| IndexError::Backend {
        operation,
        cause: Box::new(error),
    }
}

/// Which constraint a write ran into, if it ran into one.
///
/// The catalog's constraints are not decoration: the primary key on `entries`
/// is EP-5's uniqueness written down, and the foreign key from an Entry to its
/// Container is CP-11's promise that a record carries the Containers its
/// Entries belong to. A caller is told which of the two was broken rather than
/// that "a constraint failed".
pub(crate) enum Violation {
    /// Two rows claimed one key.
    Duplicate,
    /// A row referred to something that is not there.
    Missing,
    /// Not a constraint failure at all.
    None,
}

/// Reads what a failed write ran into.
pub(crate) fn violation(error: &rusqlite::Error) -> Violation {
    let rusqlite::Error::SqliteFailure(failure, _) = error else {
        return Violation::None;
    };
    match failure.extended_code {
        ffi::SQLITE_CONSTRAINT_PRIMARYKEY | ffi::SQLITE_CONSTRAINT_UNIQUE => Violation::Duplicate,
        ffi::SQLITE_CONSTRAINT_FOREIGNKEY => Violation::Missing,
        _ => Violation::None,
    }
}

/// A stored value spelled in a vocabulary this build has no reading for.
///
/// It is what [`IndexError::UnreadableCatalog`] carries as its cause, so what
/// was expected and what was in the file are both reported rather than guessed
/// at.
#[derive(Debug)]
pub(crate) struct UnreadableValue {
    expected: &'static str,
    found: String,
}

impl UnreadableValue {
    pub(crate) fn new(expected: &'static str, found: impl Into<String>) -> Self {
        Self {
            expected,
            found: found.into(),
        }
    }
}

impl fmt::Display for UnreadableValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is not a {} this build knows",
            self.found, self.expected
        )
    }
}

impl error::Error for UnreadableValue {}

/// A local path as the text a column holds.
///
/// Paths are stored as text, and a filesystem may hand out a name that is not
/// valid UTF-8. Such a path is refused as
/// [`IndexError::UnrepresentablePath`] — a refusal of what was handed in, not a
/// failure of the store.
pub(crate) fn path_text<'a>(path: &'a Path, operation: &'static str) -> IndexResult<&'a str> {
    path.to_str()
        .ok_or_else(|| IndexError::UnrepresentablePath {
            operation,
            path: path.to_path_buf(),
        })
}

/// A value the catalog holds that this build cannot read back.
pub(crate) fn unreadable(
    operation: &'static str,
    expected: &'static str,
    found: impl Into<String>,
) -> IndexError {
    IndexError::UnreadableCatalog {
        operation,
        cause: Box::new(UnreadableValue::new(expected, found)),
    }
}

/// A value the catalog holds that the domain does not admit.
///
/// The same answer as [`unreadable`] — the file is not one this build can read
/// — with the domain's own refusal carried as the structured cause, and with
/// what the Index was doing alongside it, which the model error alone cannot
/// say.
pub(crate) fn unreadable_model(
    operation: &'static str,
) -> impl Fn(coffret_model::Error) -> IndexError {
    move |error| IndexError::UnreadableCatalog {
        operation,
        cause: Box::new(error),
    }
}
