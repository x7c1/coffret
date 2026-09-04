use std::fmt;

use crate::PathDefect;

mod parse;

mod stored;

#[cfg(test)]
mod tests;

/// The Library position an Entry occupies.
///
/// Both rules an Entry Path is held to are this type's invariants, so nothing
/// downstream has to ask whether the path it holds was checked by whoever built
/// it:
///
/// - Every Entry Path is Unicode normalized to NFC (spec: EP-1). Past the
///   normal form the path is opaque and kept verbatim — it composes nothing
///   further and folds nothing together (spec: EP-3).
/// - Every Entry Path is in the shape EP-2 spells: non-empty, relative to the
///   Library root, made of components separated by `/` and by nothing else,
///   with no empty component, no `.` or `..` component, no NUL, and no
///   separator at either end. So there is no `EntryPath` that names something
///   outside the Library, and no caller has to re-check one it was handed.
///
/// There is no way to build one without saying which side of the Library
/// boundary the text came from, because the two sides owe different answers:
///
/// - [`parse`](Self::parse) is for text from outside — a name a filesystem
///   handed a scan back, the top-level component a device's mapping is
///   configured with, a prefix a caller narrows a run to. One filesystem spells
///   `é` as a single code point and another as `e` followed by a combining
///   acute, for the very same file, so the spelling becomes the Library's on the
///   way in; the shape is then checked, and text outside it is refused with the
///   part of the shape it failed.
/// - [`stored`](Self::stored) is for text the Library already holds — a decoded
///   meta section, a control payload, a catalog row. EP-1 and EP-2 already hold
///   of those, so one that is not NFC is malformed data and is refused rather
///   than rewritten: composing it would change bytes a digest was taken over and
///   make a stored record decode to something other than what was encoded. One
///   outside the shape is malformed for the plainer reason that nothing holding
///   to EP-2 could have written it.
///
/// Joining is the one construction that owes no check:
/// [`below`](Self::below) puts two of these together, and two shaped paths
/// joined by `/` are a shaped path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryPath(String);

impl EntryPath {
    /// The path `relative` stands at when it is read as a position under this
    /// one.
    ///
    /// The one construction with nothing to refuse: both halves are already NFC
    /// and already in EP-2's shape, and joining them with the one separator
    /// leaves a path that is both — no component of either became empty,
    /// relative, or a NUL by being written after the other. Callers that would
    /// otherwise rebuild a path out of its text and parse it again use this, so
    /// that a join has no failure mode to invent an answer for.
    pub fn below(&self, relative: &Self) -> Self {
        Self(format!("{}/{}", self.0, relative.0))
    }

    /// The folder this path stands in, or `None` where it stands directly in
    /// the Library root.
    ///
    /// Everything before the last separator and nothing cleverer: `/` is an
    /// Entry Path's only logical separator (spec: EP-2), so the cut is
    /// unambiguous, and what is left of a shaped path when a whole trailing
    /// component is taken off is a shaped path — which is why this answers
    /// without a refusal to report.
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once('/')
            .map(|(folder, _)| Self(folder.to_owned()))
    }

    /// The last component of the path, which is what an Entry or a folder is
    /// called where it stands.
    ///
    /// The whole path where it has only one component, for the reason
    /// [`top_level`](Self::top_level) is: `/` is the only logical separator
    /// (spec: EP-2). The dual of [`parent`](Self::parent) — the two together
    /// are the whole path.
    pub fn name(&self) -> &str {
        match self.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => &self.0,
        }
    }

    /// The path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first component of the path, which is what a device's mappings are
    /// keyed by (spec: EP-9).
    ///
    /// The whole path where it has only one component, since `/` is the only
    /// logical separator (spec: EP-2).
    pub fn top_level(&self) -> &str {
        self.0.split('/').next().unwrap_or(&self.0)
    }

    /// Whether this path is `prefix` itself or lies beneath it.
    ///
    /// A prefix covers the Entry at exactly that path and everything under
    /// `prefix/`, and nothing else: `books` never covers
    /// `books-annex/page-1.png`, because `/` is the only logical separator
    /// (spec: EP-2, EP-9). It lives here, and not in each caller, because a scan
    /// narrowing to a mapping, a catalog answering `entries_under`, and a fetch
    /// narrowing to a subtree all have to agree on where a subtree ends.
    pub fn is_under(&self, prefix: &Self) -> bool {
        self.0 == prefix.0
            || self
                .0
                .strip_prefix(&prefix.0)
                .is_some_and(|rest| rest.starts_with('/'))
    }
}

impl fmt::Display for EntryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What is wrong with a piece of text that has to be an Entry Path, if anything
/// is (spec: EP-2).
///
/// Both constructors end here, which is what makes the shape one rule rather
/// than two that could drift apart: text from outside is composed and then
/// asked this, and stored text is asked this as it stands.
///
/// The separators at either end are looked at before the components are, so
/// that a path with one of them is told about that rather than about the empty
/// component it leaves behind — which is the same fact stated where nobody can
/// act on it.
fn defect_in(text: &str) -> Option<PathDefect> {
    if text.is_empty() {
        return Some(PathDefect::Empty);
    }
    if text.contains('\0') {
        return Some(PathDefect::Nul);
    }
    if text.starts_with('/') {
        return Some(PathDefect::LeadingSeparator);
    }
    if text.ends_with('/') {
        return Some(PathDefect::TrailingSeparator);
    }
    for component in text.split('/') {
        if component.is_empty() {
            return Some(PathDefect::EmptyComponent);
        }
        if component == "." || component == ".." {
            return Some(PathDefect::RelativeComponent);
        }
    }
    None
}
