//! The one place the fixture literals become Entry Paths.
//!
//! Every Entry Path is built by reading text (spec: EP-1, EP-2), and the
//! generator's paths are text this crate wrote itself: `albums/00/cover.jpg` is
//! in the source above, not in anything a run was handed. So a refusal here is a
//! typo in the fixture set rather than a state any exchange could reach, and it
//! is said as one — which is what keeps the failure out of every builder the
//! path passes through on its way into an object.

use coffret_model::EntryPath;

/// The Entry Path a fixture literal spells, or a panic naming the literal that
/// spells none.
pub(super) fn entry_path(text: impl Into<String>) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal this crate wrote: {error}"))
}
