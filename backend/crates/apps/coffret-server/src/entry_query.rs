use coffret_device::{EntryPath, ModelError, PathDefect, Redacted};
use serde::Deserialize;

use crate::api_error::ApiError;

/// The `?path=` a route was asked with.
///
/// The path is a query parameter rather than a path segment for one reason: an
/// Entry Path's only logical separator is `/` (spec: EP-2), and a path segment
/// would have to escape every one of them — which means every caller has to
/// escape correctly and every proxy in between has to leave the escaping alone.
/// A query parameter carries the separator as itself.
#[derive(Debug, Deserialize)]
pub struct PathQuery {
    path: Option<String>,
}

impl PathQuery {
    /// The folder the query names, `None` being the Library root.
    pub fn folder(&self) -> Result<Option<EntryPath>, ApiError> {
        folder_named(self.path.as_deref())
    }

    /// The Entry the query names.
    ///
    /// There is no root to fall back on here: an Entry Path is non-empty
    /// (spec: EP-2), so a request for the bytes at no path is a request for
    /// nothing.
    pub fn entry(&self) -> Result<EntryPath, ApiError> {
        match self.path.as_deref() {
            // The same words the model refuses an empty path in, because it is
            // the same refusal: the route decides only that a folder may be
            // empty and an Entry may not.
            None | Some("") => Err(ApiError::bad_path(PathDefect::Empty)),
            Some(text) => shaped(text),
        }
    }
}

/// The folder a `?path=` names, `None` being the Library root.
///
/// An absent parameter and an empty one are the same answer, because they are
/// the same intention: the root is what a caller that named no folder means, and
/// it is what an explorer's first request carries before anything has been
/// chosen.
///
/// A function beside [`PathQuery`] rather than only a method on it, because the
/// upload takes a second parameter of its own and so reads `?path=` out of a
/// query of its own — and two readings of what a folder parameter means would be
/// two answers to what the Library root is spelled as.
pub(crate) fn folder_named(path: Option<&str>) -> Result<Option<EntryPath>, ApiError> {
    match path {
        None | Some("") => Ok(None),
        Some(text) => shaped(text).map(Some),
    }
}

/// One piece of text from outside the Library, as an Entry Path.
///
/// The reading is [`EntryPath::parse`]'s (spec: EP-1, EP-2). What this route
/// settles is which of its answers a caller hears about: composing the text to
/// NFC is not a refusal — a caller whose keyboard produced the decomposed
/// spelling asked for the file that is there, and answering `400` would be
/// telling them their own filename is malformed — while a shape EP-2 excludes
/// is, and the refusal carries which part of it went, because a caller told
/// only that a path was refused has no way to find the one component that made
/// it so.
pub(crate) fn shaped(text: &str) -> Result<EntryPath, ApiError> {
    EntryPath::parse(text).map_err(|error| match error {
        ModelError::MalformedEntryPath { defect, .. } => ApiError::bad_path(defect),
        // `parse` refuses for that one reason and no other, so a second one
        // would be a rule this route has never been told about — and a refusal
        // it cannot read is not one it can hand a caller as their own doing.
        // It goes where every failure this crate has no reading for goes: what
        // was said reaches the log, and the caller is told only that the server
        // could not answer.
        other => ApiError::server(other.redacted()),
    })
}
