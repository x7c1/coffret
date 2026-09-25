use std::collections::BTreeMap;

use coffret_model::EntryPath;

use crate::local_scan::root_state::RootState;
use crate::local_scan::source_file::SourceFile;
use crate::local_scan::walked_root::WalkedRoot;
use crate::unavailable_root::UnavailableRoot;

/// What one walk of every mapping found, and what it made of each root.
pub(crate) struct Walked {
    /// Every regular file under every available mapping, by the Entry Path it
    /// stands at.
    pub(crate) found: BTreeMap<EntryPath, SourceFile>,
    /// One verdict per mapping, in the order the mappings were given.
    pub(crate) roots: Vec<WalkedRoot>,
}

/// The mappings whose roots the device could not vouch for, in mapping order
/// (spec: EP-12).
///
/// Both flows report the same finding out of the same verdicts, which is why the
/// finding is named once at the crate root — so the reading of the verdicts is
/// here rather than spelled out once per flow.
///
/// The prefix leaves this crate as an [`EntryPath`] — a Library position a
/// caller may hold against the paths a run reports. The local root is a local
/// path and normalizes nowhere: what the operating system was given is what it
/// is named by.
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
