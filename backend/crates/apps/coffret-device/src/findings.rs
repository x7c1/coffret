use coffret_usecase::fetch::{EntryFetch, FetchOutcome, Surfaced as Declined};
use coffret_usecase::freeze::{FreezeOutcome, NotFrozen};
use coffret_usecase::sync::{Surfaced, SyncOutcome};
use coffret_usecase::UnavailableRoot;

use crate::finding::Finding;
use crate::finding_reason::FindingReason;

/// What a run that succeeded still has to be read for.
///
/// Every one of the four outcomes says the same thing in its own words: a run
/// that returns `Ok` has not necessarily backed up or placed everything, and the
/// lists saying what it left alone are not optional reading (spec: PK-14,
/// EP-11, EP-12). A caller that reads only the counts would tell a person their
/// folder is safe when it is not, so this is the one view over all four lists —
/// built the same way for the command line and for the explorer, because a
/// finding one of them showed and the other swallowed would be worse than
/// either.
///
/// [`needs_attention`](Self::needs_attention) is the whole of the verdict: a
/// run whose findings are all settled batches did everything it was asked to,
/// and only reports what it tidied on the way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Findings(Vec<Finding>);

impl Findings {
    /// Whether the run reported nothing at all.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether the run left anything for somebody to act on.
    pub fn needs_attention(&self) -> bool {
        self.0.iter().any(Finding::needs_attention)
    }

    /// How many findings the run reported, settled batches among them.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Each finding, in the order the run reported it.
    pub fn iter(&self) -> std::slice::Iter<'_, Finding> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a Findings {
    type Item = &'a Finding;
    type IntoIter = std::slice::Iter<'a, Finding>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl From<&SyncOutcome> for Findings {
    fn from(outcome: &SyncOutcome) -> Self {
        let surfaced = outcome.surfaced.iter().map(|surfaced| match surfaced {
            Surfaced::PackResident { path, .. } => Finding::Surfaced {
                path: path.clone(),
                reason: FindingReason::ChangedInPack,
            },
            Surfaced::DeletedLocally { path } => Finding::Surfaced {
                path: path.clone(),
                reason: FindingReason::DeletedLocally,
            },
        });
        let settled = outcome.reconciled.iter().cloned().map(Finding::Settled);

        Self(
            surfaced
                .chain(unavailable(&outcome.unavailable))
                .chain(settled)
                .collect(),
        )
    }
}

impl From<&FreezeOutcome> for Findings {
    fn from(outcome: &FreezeOutcome) -> Self {
        let surfaced = outcome.surfaced.iter().map(|surfaced| match surfaced {
            NotFrozen::ModifiedInPack { path, .. } => Finding::Surfaced {
                path: path.clone(),
                reason: FindingReason::ChangedInPack,
            },
            NotFrozen::KeyLostInPack { path, .. } => Finding::Surfaced {
                path: path.clone(),
                reason: FindingReason::KeyLost,
            },
        });

        Self(surfaced.chain(unavailable(&outcome.unavailable)).collect())
    }
}

impl From<&FetchOutcome> for Findings {
    fn from(outcome: &FetchOutcome) -> Self {
        let locked = outcome
            .locked
            .iter()
            .map(|container_id| Finding::LockedContainer {
                container_id: *container_id,
            });

        Self(
            outcome
                .surfaced
                .iter()
                .map(declined)
                .chain(locked)
                .collect(),
        )
    }
}

impl From<&EntryFetch> for Findings {
    fn from(fetch: &EntryFetch) -> Self {
        match fetch {
            // A Container this run read a range out of is exactly as unfetched
            // afterwards as it was before (spec: PK-16), so there is no locked
            // Container to report here even where the one Entry was locked: the
            // finding is about the Entry that was asked for.
            EntryFetch::Placed | EntryFetch::AlreadyPresent => Self::default(),
            EntryFetch::Surfaced(surfaced) => Self(vec![declined(surfaced)]),
        }
    }
}

/// The finding for one Entry a fetch declined to place.
fn declined(surfaced: &Declined) -> Finding {
    let reason = match surfaced {
        Declined::ForeignFile { .. } => FindingReason::ForeignFile,
        Declined::LocallyChanged { .. } => FindingReason::LocallyChanged,
        Declined::WitnessedDeletion { .. } => FindingReason::WitnessedDeletion,
        Declined::UnreachablePlace { component, .. } => FindingReason::UnreachablePlace {
            component: component.clone(),
        },
        Declined::KeyLost { .. } => FindingReason::KeyLost,
    };
    Finding::Surfaced {
        path: surfaced.path().clone(),
        reason,
    }
}

/// The findings for the mappings a run could not vouch for (spec: EP-12).
fn unavailable(roots: &[UnavailableRoot]) -> impl Iterator<Item = Finding> + '_ {
    roots.iter().map(|root| Finding::UnavailableRoot {
        local_root: root.local_root.clone(),
        reason: root.reason,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use coffret_model::{ContainerId, EntryPath};
    use coffret_usecase::sync::Reconciled;
    use coffret_usecase::RootUnavailable;

    use super::*;

    // PK-14 and EP-12 are one obligation to whoever asked for the run: what was
    // left alone, and what was never looked at. Both have to come out of one
    // reading, or a caller shows one and swallows the other.
    #[test]
    fn a_sync_reports_what_it_left_alone_and_what_it_could_not_read() {
        let outcome = SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 0,
            surfaced: vec![Surfaced::DeletedLocally {
                path: EntryPath::nfc("albums/gone.jpg"),
            }],
            unavailable: vec![UnavailableRoot {
                prefix: None,
                local_root: PathBuf::from("/mnt/photos"),
                reason: RootUnavailable::Missing,
            }],
            reconciled: Vec::new(),
            commit: None,
        };

        let findings = Findings::from(&outcome);
        assert!(!findings.is_empty());
        assert!(findings.needs_attention());
        assert_eq!(findings.len(), 2);

        let rendered: Vec<String> = findings.iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered[0],
            "surfaced albums/gone.jpg: this device had it and it is gone from disk"
        );
        assert_eq!(rendered[1], "unavailable root /mnt/photos: it is not there");
    }

    // A run with nothing to report is the only run a caller may read as "every
    // file is where the Library says it is".
    #[test]
    fn a_run_that_left_nothing_alone_has_no_findings() {
        let outcome = FetchOutcome {
            fetched: vec![EntryPath::nfc("albums/kept.jpg")],
            containers: Vec::new(),
            skipped: 0,
            surfaced: Vec::new(),
            locked: Vec::new(),
        };

        assert!(Findings::from(&outcome).is_empty());
    }

    // A batch an interrupted run left behind is reported because the run
    // finished it (spec: OC-7), not because anyone has to: it is the one
    // finding that leaves nothing for the person who asked.
    #[test]
    fn a_run_that_only_settled_a_leftover_batch_needs_no_attention() {
        let outcome = SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 3,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            reconciled: vec![Reconciled::Completed {
                container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
                entries: 1,
            }],
            commit: None,
        };

        let findings = Findings::from(&outcome);
        assert_eq!(findings.len(), 1);
        assert!(!findings.is_empty());
        assert!(!findings.needs_attention());
    }

    // KL-7 is a loss at the Container level, and the fetch reports it at both
    // levels for that reason: one marker locks every Entry the Container holds.
    #[test]
    fn a_fetch_reports_a_locked_container_as_well_as_its_entries() {
        let container_id = ContainerId::from_bytes([7; ContainerId::BYTE_LEN]);
        let outcome = FetchOutcome {
            fetched: Vec::new(),
            containers: Vec::new(),
            skipped: 0,
            surfaced: vec![Declined::KeyLost {
                path: EntryPath::nfc("albums/locked.jpg"),
                container_id,
            }],
            locked: vec![container_id],
        };

        let rendered: Vec<String> = Findings::from(&outcome)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            rendered,
            [
                "surfaced albums/locked.jpg: the Library records no key for the Container holding \
                 it"
                .to_owned(),
                format!("locked container {container_id}"),
            ]
        );
    }

    // EP-11 reports every Entry a fetch declined with the reason it was
    // declined, on the no-silent-selection posture EP-4 sets. This one names
    // the folder as well, because looking at that one name is what a person
    // does next — and the run placed everything the folder says nothing about.
    #[test]
    fn a_place_the_run_could_not_reach_is_a_finding_that_names_the_folder() {
        let outcome = FetchOutcome {
            fetched: vec![EntryPath::nfc("albums/spring.jpg")],
            containers: Vec::new(),
            skipped: 0,
            surfaced: vec![Declined::UnreachablePlace {
                path: EntryPath::nfc("link/authorized_keys"),
                component: PathBuf::from("/home/someone/mapped/link"),
            }],
            locked: Vec::new(),
        };

        let findings = Findings::from(&outcome);
        assert!(findings.needs_attention());

        let rendered: Vec<String> = findings.iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered[0],
            "surfaced link/authorized_keys: a folder on the way to it is not a folder of the \
             mapped folder — /home/someone/mapped/link"
        );
        assert_eq!(rendered.len(), 1, "the Entry that was placed is not one");
    }

    // One Entry that was placed is the whole answer: there is nothing for the
    // caller to act on.
    #[test]
    fn one_entry_placed_reports_nothing() {
        assert!(Findings::from(&EntryFetch::Placed).is_empty());
        assert!(Findings::from(&EntryFetch::AlreadyPresent).is_empty());
        assert_eq!(
            Findings::from(&EntryFetch::Surfaced(Declined::ForeignFile {
                path: EntryPath::nfc("albums/theirs.jpg"),
            }))
            .len(),
            1
        );
    }
}
