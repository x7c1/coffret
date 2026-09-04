//! The one place this crate's tests and conformance suites turn a literal into
//! an [`EntryPath`].
//!
//! Every Entry Path is built by parsing text (spec: EP-1, EP-2), and a fixture
//! is no exception: what a suite writes down is text like any other, and the
//! type has no constructor that takes it on trust. The unwrap lives here rather
//! than at each of the fixtures so that a literal somebody mistypes is reported
//! once, as the mistake in the fixture that it is.

use coffret_model::EntryPath;

/// The Entry Path `text` spells, or a panic naming the literal that does not
/// spell one.
pub(crate) fn entry_path(text: impl Into<String>) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}
