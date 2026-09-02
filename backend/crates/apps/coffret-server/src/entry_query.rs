use coffret_device::EntryPath;
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
            None | Some("") => Err(ApiError::bad_path("it is empty")),
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
/// Composed first and checked second, and the order is the rule's. Text from
/// outside is normalized to NFC on the way in (spec: EP-1) — one filesystem
/// spells `é` as one code point and another as two, and a browser sends back
/// whichever its platform kept — so composing it is what makes it the same path
/// the Library holds rather than a path that merely looks like it. It is not a
/// refusal for the same reason: a caller whose keyboard produced the decomposed
/// spelling asked for the file that is there, and answering `400` would be
/// telling them their own filename is malformed.
///
/// The shape is then EP-2's, and a failure names which part of it went — because
/// a caller told only that a path was refused has no way to find the one
/// component that made it so.
pub(crate) fn shaped(text: &str) -> Result<EntryPath, ApiError> {
    let path = EntryPath::nfc(text);
    match defect_in(path.as_str()) {
        Some(defect) => Err(ApiError::bad_path(defect)),
        None => Ok(path),
    }
}

/// What is wrong with a piece of text that has to be an Entry Path
/// (spec: EP-2).
///
/// The separators are looked at before the components are, so that a path with
/// one on either end is told about that rather than about the empty component it
/// leaves behind — which is the same fact stated where nobody can act on it.
fn defect_in(text: &str) -> Option<&'static str> {
    if text.is_empty() {
        return Some("it is empty");
    }
    if text.contains('\0') {
        return Some("it holds a NUL");
    }
    if text.starts_with('/') {
        return Some(
            "it begins with a separator, and an Entry Path is relative to the Library root",
        );
    }
    if text.ends_with('/') {
        return Some("it ends with a separator");
    }
    for component in text.split('/') {
        if component.is_empty() {
            return Some("it holds an empty component");
        }
        if component == "." || component == ".." {
            return Some("it holds a `.` or `..` component");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::defect_in;

    // EP-2: what an Entry Path may not be. `..` is the one worth naming twice —
    // a path that climbed out of a mapped folder would be a path the Library
    // never held, and it is refused here rather than anywhere further in.
    #[test]
    fn every_shape_ep_2_excludes_is_refused() {
        for text in [
            "",
            "/albums/spring.jpg",
            "albums/spring.jpg/",
            "albums//spring.jpg",
            "albums/../../etc/passwd",
            "..",
            "albums/./spring.jpg",
            "albums/spring\0.jpg",
        ] {
            assert!(defect_in(text).is_some(), "{text:?} is not an Entry Path");
        }
    }

    #[test]
    fn an_ordinary_path_is_not_refused() {
        for text in ["notes.txt", "albums/2026/08/spring.jpg", "books-annex"] {
            assert_eq!(defect_in(text), None, "{text:?} is an Entry Path");
        }
    }

    // A component that merely begins or ends with a dot is a name, not a
    // relative reference: `.hidden` and `..trailing` are files somebody has.
    #[test]
    fn a_name_that_starts_with_a_dot_is_a_name() {
        assert_eq!(defect_in("albums/.hidden"), None);
        assert_eq!(defect_in("albums/...three"), None);
    }
}
