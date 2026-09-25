//! The state one mapped root is in, as far as a walk may read it as evidence.
//!
//! This is the verdict alone. The question that reaches it is
//! [`assess_root`](super::assess_root), and what the walk hands back for each
//! mapping — the mapping and this verdict together — is
//! [`WalkedRoot`](super::WalkedRoot).

use crate::device_state::RootIdentity;
use crate::unavailable_root::RootUnavailable;

/// What one mapped root turned out to be, before anything under it was read
/// (spec: EP-12).
pub(crate) enum RootState {
    /// The root is there and stands on the filesystem the mapping records — or
    /// on one this platform can say nothing about, which leaves the mapping
    /// guarded by the root's existence alone.
    Available,
    /// The identity to stamp the mapping with: the root is there, and either the
    /// mapping records no filesystem at all — nothing to compare against, so
    /// what the root holds decides nothing — or it records a different one and
    /// the root holds files.
    Stamp(RootIdentity),
    /// Nothing under the root is evidence about anything.
    Unavailable(RootUnavailable),
}
