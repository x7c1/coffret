//! What the walk found one mapped root to be: the mapping, and the
//! [`RootState`] its root was in when it was asked.
//!
//! The state is a verdict about a root and names no mapping; this is that
//! verdict carried beside the mapping it is about, which is what a flow needs
//! to re-stamp one or to report it unavailable.

use crate::device_state::Mapping;
use crate::local_scan::root_state::RootState;

/// One mapping, and what the walk found its root to be.
pub(crate) struct WalkedRoot {
    /// The mapping exactly as the device recorded it, which is at once the key a
    /// re-stamp writes back under and the spelling the walk composed its Entry
    /// Paths from: a mapping's prefix is an [`EntryPath`] and so exists only in
    /// NFC (spec: EP-1), leaving the recorded key and the subtree the walk
    /// claims one string rather than two.
    ///
    /// [`EntryPath`]: coffret_model::EntryPath
    pub(crate) mapping: Mapping,
    /// What the root was found to be, before anything under it was read.
    pub(crate) state: RootState,
}
