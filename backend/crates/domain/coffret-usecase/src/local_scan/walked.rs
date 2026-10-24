use std::collections::BTreeMap;

use coffret_model::EntryPath;

use crate::local_scan::root_state::RootState;
use crate::local_scan::source_file::SourceFile;
use crate::local_scan::stale_scratch::StaleScratch;
use crate::local_scan::system_file::SystemFile;
use crate::local_scan::walked_root::WalkedRoot;
use crate::unavailable_root::UnavailableRoot;

/// What one walk of every mapping found, and what it made of each root.
pub(crate) struct Walked {
    /// Every regular file under every available mapping, by the Entry Path it
    /// stands at.
    pub(crate) found: BTreeMap<EntryPath, SourceFile>,
    /// One verdict per mapping, in the order the mappings were given.
    pub(crate) roots: Vec<WalkedRoot>,
    /// Every scratch of coffret's own shape the walk met that no writer in
    /// this process holds (spec: OC-8, EP-11). Nothing in it is among
    /// [`found`](Self::found): a scratch is never a file to back up.
    pub(crate) stale: Vec<StaleScratch>,
    /// Every name the walk left out because an operating system keeps it, in
    /// Entry Path order (spec: EP-17). Nothing in it is among
    /// [`found`](Self::found), and nothing under a folder in it was walked.
    pub(crate) system: Vec<SystemFile>,
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
