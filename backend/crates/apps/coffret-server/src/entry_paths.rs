//! The one place this crate's tests turn a literal into an [`EntryPath`].
//!
//! Every Entry Path is built by parsing text (spec: EP-1, EP-2), and a fixture
//! is no exception: what a test writes down is text like any other, and the
//! type has no constructor that takes it on trust.

use coffret_device::EntryPath;

/// The Entry Path `text` spells, or a panic naming the literal that does not
/// spell one.
pub(crate) fn entry_path(text: impl Into<String>) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}
