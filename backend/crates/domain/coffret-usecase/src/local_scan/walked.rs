use std::collections::BTreeMap;

use coffret_model::EntryPath;

use crate::device_state::{Mapping, RootIdentity};
use crate::local_scan::source_file::SourceFile;
use crate::unavailable_root::{RootUnavailable, UnavailableRoot};

/// What one walk of every mapping found, and what it made of each root.
pub(crate) struct Walked {
    /// Every regular file under every available mapping, by the Entry Path it
    /// stands at.
    pub(crate) found: BTreeMap<EntryPath, SourceFile>,
    /// One verdict per mapping, in the order the mappings were given.
    pub(crate) roots: Vec<WalkedRoot>,
}

/// One mapping, and what the walk found its root to be.
pub(crate) struct WalkedRoot {
    pub(crate) mapping: Mapping,
    pub(crate) state: RootState,
}

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

/// The mappings the walk could not vouch for, in mapping order (spec: EP-12).
///
/// Both flows report the same finding out of the same verdicts, which is why the
/// finding is named once at the crate root — so the reading of the verdicts is
/// here rather than spelled out once per flow.
pub(crate) fn unavailable_roots(roots: &[WalkedRoot]) -> Vec<UnavailableRoot> {
    roots
        .iter()
        .filter_map(|root| match root.state {
            RootState::Unavailable(reason) => Some(UnavailableRoot {
                prefix: root.mapping.prefix.clone(),
                local_root: root.mapping.local_root.clone(),
                reason,
            }),
            RootState::Available | RootState::Stamp(_) => None,
        })
        .collect()
}
