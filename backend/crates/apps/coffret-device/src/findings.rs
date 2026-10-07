use std::error;

use coffret_usecase::commit::{CheckpointOutcome, CommitFailure, CommitOutcome, DegradedKeyring};
use coffret_usecase::fetch::{EntryFetch, EntryFetchOutcome, FetchOutcome, Surfaced as Declined};
use coffret_usecase::freeze::{FreezeOutcome, NotFrozen};
use coffret_usecase::sync::{Surfaced, SyncOutcome};
use coffret_usecase::{RefusedRoot, UnavailableRoot};

use crate::finding::{chained, Finding};
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
/// run whose findings are all settled batches, untrashed removals and
/// checkpoints not written did everything it was asked to, and only reports
/// what it tidied on the way and what its commit left for later.
///
/// No `PartialEq`, for the reason [`Finding`] has none: one of them carries a
/// refused root's reason, and error values are reported rather than compared.
#[derive(Debug, Clone, Default)]
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

    /// Findings put together by hand rather than read off an outcome.
    ///
    /// For another crate's cases alone: a shell's own tests have to say what it
    /// does with a run's findings, and an outcome is a use-case value no shell
    /// can assemble — a commit outcome carries a Journal record. Behind a
    /// feature that only `coffret-cli`'s `[dev-dependencies]` turn on, which
    /// under resolver 2 never reaches a normal build, so no shipping caller can
    /// report findings a run did not.
    #[cfg(feature = "assembled-findings")]
    pub fn assembled(findings: impl IntoIterator<Item = Finding>) -> Self {
        Self(findings.into_iter().collect())
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
        let settled = outcome.settled.iter().cloned().map(Finding::Settled);

        Self(
            surfaced
                .chain(unavailable(&outcome.unavailable))
                .chain(settled)
                .chain(committed(outcome.commit.as_ref()))
                .chain(repaired(outcome.commit.as_ref()))
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

        Self(
            surfaced
                .chain(unavailable(&outcome.unavailable))
                .chain(committed(outcome.commit.as_ref()))
                .chain(repaired(outcome.commit.as_ref()))
                .chain(degraded(outcome.degraded.as_ref()))
                .collect(),
        )
    }
}

impl From<&FetchOutcome> for Findings {
    fn from(outcome: &FetchOutcome) -> Self {
        let key_lost = outcome
            .key_lost
            .iter()
            .map(|container_id| Finding::KeyLostContainer {
                container_id: *container_id,
            });

        Self(
            outcome
                .surfaced
                .iter()
                .map(declined)
                .chain(refused(&outcome.refused))
                .chain(key_lost)
                .chain(degraded(outcome.degraded.as_ref()))
                .collect(),
        )
    }
}

impl From<&EntryFetchOutcome> for Findings {
    fn from(outcome: &EntryFetchOutcome) -> Self {
        let entry = match &outcome.fetch {
            // A Container this run read a range out of is exactly as unfetched
            // afterwards as it was before (spec: PK-16), so there is no key-lost
            // Container to report here even where the one Entry was unreadable: the
            // finding is about the Entry that was asked for.
            EntryFetch::Placed | EntryFetch::AlreadyPresent => None,
            EntryFetch::Surfaced(surfaced) => Some(declined(surfaced)),
        };
        Self(
            entry
                .into_iter()
                .chain(degraded(outcome.degraded.as_ref()))
                .collect(),
        )
    }
}

/// The findings for the Keyring repairs a commit performed before it failed
/// (spec: KL-15).
///
/// A run that failed answers with its error and not with an outcome, and the
/// replicas its commit put back stand on Storage whatever became of the batch,
/// so these are the whole of what a failed run still has to say beside its
/// refusal.
impl From<&CommitFailure> for Findings {
    fn from(failure: &CommitFailure) -> Self {
        Self(failure.repairs.iter().map(Finding::from).collect())
    }
}

impl Findings {
    /// The findings a run that failed with `error` still has to say: the
    /// Keyring repairs its commit performed before it failed, found wherever in
    /// the chain the commit's failure is, and none where there is no such
    /// failure (spec: KL-15).
    ///
    /// Every shell reads a failed run through this, so a repair a run performed
    /// is said on the failure path in the words it is said on success.
    pub fn repaired_before(error: &(dyn error::Error + 'static)) -> Self {
        std::iter::successors(Some(error), |link| link.source())
            .find_map(|link| link.downcast_ref::<CommitFailure>())
            .map(Self::from)
            .unwrap_or_default()
    }
}

/// The findings for the committed Keyring generations a commit repaired, one
/// each, and none where it repaired nothing (spec: KL-13, KL-15).
///
/// Last among a run's findings, beside a degraded Keyring, because it is about
/// the Library rather than about anything the run was asked to do: a screen
/// with room for one finding shows one somebody has to act on first. One per
/// generation, because a run that repaired a set and then rebased onto another
/// device's head repaired *that* generation too if it was short.
fn repaired(commit: Option<&CommitOutcome>) -> impl Iterator<Item = Finding> + '_ {
    commit
        .into_iter()
        .flat_map(|commit| commit.repairs.iter().map(Finding::from))
}

/// The finding for a committed Keyring set a run's read had to step over a
/// position of, where nothing later in the run spoke for it (spec: KL-5, KL-15).
///
/// Last among a run's findings, because it is about the Library rather than
/// about anything the run was asked to do.
fn degraded(found: Option<&DegradedKeyring>) -> Option<Finding> {
    found.map(Finding::from)
}

/// The finding for one Entry a fetch declined to place.
fn declined(surfaced: &Declined) -> Finding {
    let reason = match surfaced {
        Declined::ForeignFile { .. } => FindingReason::ForeignFile,
        Declined::LocallyChanged { .. } => FindingReason::LocallyChanged,
        Declined::WitnessedDeletion { .. } => FindingReason::WitnessedDeletion,
        Declined::UnreachablePlace { stopped_at, .. } => FindingReason::UnreachablePlace {
            stopped_at: stopped_at.clone(),
        },
        Declined::KeyLost { .. } => FindingReason::KeyLost,
        Declined::ReservedComponent { .. } => FindingReason::ReservedComponent,
    };
    Finding::Surfaced {
        path: surfaced.path().clone(),
        reason,
    }
}

/// The findings for what a commit could not finish after its record
/// (spec: CP-1): each removal Storage would not trash, and a checkpoint that
/// could not be written.
///
/// The commit outcome reports both so that they are not lost in a diagnostic
/// event, and this is where they reach the person who asked for the run. A
/// checkpoint that was not due, was written, or was found already written by
/// another writer is nothing to say.
fn committed(commit: Option<&CommitOutcome>) -> Vec<Finding> {
    let Some(commit) = commit else {
        return Vec::new();
    };
    let untrashed = commit
        .untrashed
        .iter()
        .map(|removal| Finding::UntrashedRemoval {
            container_id: removal.container_id,
            cause: removal.cause.clone(),
        });
    let checkpoint = match &commit.checkpoint {
        CheckpointOutcome::Failed { cause } => Some(Finding::CheckpointFailed {
            cause: chained(cause.as_ref()),
        }),
        CheckpointOutcome::NotDue
        | CheckpointOutcome::Written { .. }
        | CheckpointOutcome::Existing { .. } => None,
    };
    untrashed.chain(checkpoint).collect()
}

/// The findings for the mappings whose roots the device could not vouch for
/// (spec: EP-12).
fn unavailable(roots: &[UnavailableRoot]) -> impl Iterator<Item = Finding> + '_ {
    roots.iter().map(|root| Finding::UnavailableRoot {
        prefix: root.prefix.clone(),
        local_root: root.local_root.clone(),
        reason: root.reason,
    })
}

/// The findings for the mappings the device would not place into (spec: EP-13).
///
/// Beside [`unavailable`] rather than folded into it: the two are different
/// questions about a root — EP-12 asks whether it is *there to be read from*,
/// and this asks whether the folder standing at it is the one the mapping was
/// recorded against — so a caller reading the sentence is told which.
fn refused(roots: &[RefusedRoot]) -> impl Iterator<Item = Finding> + '_ {
    roots.iter().map(|root| Finding::RefusedRoot {
        prefix: root.prefix.clone(),
        local_root: root.local_root.clone(),
        reason: root.reason.clone(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use coffret_model::{
        ContainerId, Generation, JournalRecord, KeyringCommitment, MasterKeyEpoch,
    };
    use coffret_usecase::commit::{
        CommitError, KeyringRepair, RewrittenReplicas, UntrashedRemoval,
    };
    use coffret_usecase::sync::{Disposal, Settled, SyncError};
    use coffret_usecase::{root_marker, Error as StorageError, RootRefused, RootUnavailable};

    use super::*;
    use crate::testing::entry_path;

    // PK-14 and EP-12 are one obligation to whoever asked for the run: what was
    // left alone, and what was never looked at. Both have to come out of one
    // reading, or a caller shows one and swallows the other.
    #[test]
    fn a_sync_reports_what_it_left_alone_and_what_it_could_not_read() {
        let outcome = SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 0,
            mappings: 1,
            surfaced: vec![Surfaced::DeletedLocally {
                path: entry_path("albums/gone.jpg"),
            }],
            unavailable: vec![UnavailableRoot {
                prefix: None,
                local_root: PathBuf::from("/mnt/photos"),
                reason: RootUnavailable::Missing,
            }],
            settled: Vec::new(),
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
        assert_eq!(
            rendered[1],
            "unavailable root /mnt/photos, which this device maps the Library root into: it is \
             not there"
        );
    }

    // EP-12's finding names the mapping the way EP-13's does, because they are
    // two questions about one mapping and a person with several of them is
    // otherwise told a folder went unread with no way to tell which mapping it
    // belonged to. The two halves of EP-9 are said apart: a mapping for a
    // top-level component, and the one that stands for the whole Library.
    #[test]
    fn an_unavailable_root_is_a_finding_that_names_the_mapping() {
        let outcome = |prefix| SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 0,
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: vec![UnavailableRoot {
                prefix,
                local_root: PathBuf::from("/mnt/photos"),
                reason: RootUnavailable::AnotherFilesystem,
            }],
            settled: Vec::new(),
            commit: None,
        };

        let said = |prefix| {
            Findings::from(&outcome(prefix))
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
        };

        assert_eq!(
            said(Some(entry_path("albums"))),
            [
                "unavailable root /mnt/photos, which this device maps \"albums\" into: it is \
                 empty and stands on another filesystem, which is what an unmounted mount point \
                 looks like; where the folder really is empty, `coffret map` records that mapping \
                 again and the next run stamps what it finds"
                    .to_owned()
            ],
        );
        assert_eq!(
            said(None),
            [
                "unavailable root /mnt/photos, which this device maps the Library root into: it \
                 is empty and stands on another filesystem, which is what an unmounted mount \
                 point looks like; where the folder really is empty, `coffret map` records that \
                 mapping again and the next run stamps what it finds"
                    .to_owned()
            ],
            "the mapping that stands for the whole Library has no component to be named by",
        );
    }

    // The two states EP-12 names leave a person in different places, so only one
    // of them ends in a gesture: a root that is not there is a disk to plug in or
    // a share to mount, while a root that is empty on a filesystem the mapping
    // does not record is the one state a person has to act their way out of —
    // every later run reports it again until the mapping is recorded afresh.
    #[test]
    fn only_the_state_a_person_has_to_remedy_names_the_gesture() {
        let said = |reason| {
            Finding::UnavailableRoot {
                prefix: Some(entry_path("albums")),
                local_root: PathBuf::from("/mnt/photos"),
                reason,
            }
            .to_string()
        };

        let missing = said(RootUnavailable::Missing);
        assert_eq!(
            missing,
            "unavailable root /mnt/photos, which this device maps \"albums\" into: it is not \
             there",
            "there is nothing to record against a root that is not there",
        );

        let emptied = said(RootUnavailable::AnotherFilesystem);
        assert!(
            emptied.contains("`coffret map` records that mapping again"),
            "the state a run repeats forever says what remedies it: {emptied}",
        );
    }

    // A run with nothing to report is the only run a caller may read as "every
    // file is where the Library says it is".
    #[test]
    fn a_run_that_left_nothing_alone_has_no_findings() {
        let outcome = FetchOutcome {
            fetched: vec![entry_path("albums/kept.jpg")],
            containers: Vec::new(),
            skipped: 0,
            mappings: 1,
            surfaced: Vec::new(),
            refused: Vec::new(),
            key_lost: Vec::new(),
            degraded: None,
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
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            settled: vec![Settled::Completed {
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

    /// What a provider that may write and not delete answers a trash with.
    fn trash_refusal() -> StorageError {
        StorageError::PermissionDenied {
            detail: "these credentials may write but not delete".to_owned(),
            source: None,
        }
    }

    /// A settled batch, said the way the device says it.
    fn settled_said(container_id: ContainerId, disposal: Disposal) -> String {
        Finding::Settled(Settled::Disposed {
            container_id,
            disposal,
        })
        .to_string()
    }

    // Two of the three disposals leave nothing on Storage — one because the
    // earlier run never put anything there, the other because the trash took
    // it — so both are "disposed of". The third is not: Storage refused the
    // trash, the object is still there, and the sentence says so and what
    // finds it now, with Storage's own answer (spec: OC-1, OC-4).
    #[test]
    fn a_disposal_storage_refused_is_not_said_as_disposed_of() {
        let container_id = ContainerId::from_bytes([5; ContainerId::BYTE_LEN]);
        let disposed = format!(
            "settled container {container_id}: nothing committed it, so what it left was \
             disposed of"
        );

        assert_eq!(
            settled_said(container_id, Disposal::NeverUploaded),
            disposed
        );
        assert_eq!(settled_said(container_id, Disposal::Trashed), disposed);
        assert_eq!(
            settled_said(
                container_id,
                Disposal::LeftInStorage {
                    cause: trash_refusal(),
                },
            ),
            format!(
                "settled container {container_id}: nothing committed it, and Storage would not \
                 move its object to the trash (Storage refused access: these credentials may \
                 write but not delete); the object is still in Storage, and its provenance is \
                 kept for retry"
            ),
        );
    }

    /// A commit that landed, and could neither trash the one Container it
    /// removed nor write the checkpoint it was due.
    fn unfinished_commit(untrashed: ContainerId) -> CommitOutcome {
        let record = JournalRecord::new(
            Generation::FIRST,
            None,
            MasterKeyEpoch::FIRST,
            KeyringCommitment::new(Generation::FIRST, 3, "beef")
                .expect("a lowercase hex digest and a non-zero count are a valid commitment"),
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("the first record succeeds nothing");
        CommitOutcome {
            record,
            attempts: 1,
            checkpoint: CheckpointOutcome::Failed {
                cause: Box::new(CommitError::Storage(StorageError::Rejected {
                    status: 503,
                    detail: "the provider is unavailable".to_owned(),
                    source: None,
                })),
            },
            untrashed: vec![UntrashedRemoval {
                container_id: untrashed,
                cause: trash_refusal(),
            }],
            repairs: Vec::new(),
        }
    }

    // What a commit could not finish after its record is said, one line each,
    // and none of it is for somebody to act on: the committed state is correct
    // either way (spec: CP-1, OC-6, CK-8).
    #[test]
    fn what_a_commit_left_unfinished_is_said_and_not_escalated() {
        let container_id = ContainerId::from_bytes([6; ContainerId::BYTE_LEN]);
        let outcome = SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 0,
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            settled: vec![Settled::Disposed {
                container_id: ContainerId::from_bytes([4; ContainerId::BYTE_LEN]),
                disposal: Disposal::LeftInStorage {
                    cause: trash_refusal(),
                },
            }],
            commit: Some(unfinished_commit(container_id)),
        };

        let findings = Findings::from(&outcome);
        assert_eq!(findings.len(), 3);
        assert!(
            !findings.needs_attention(),
            "a disposal Storage refused, an untrashed removal and a checkpoint not written \
             leave the committed state correct",
        );

        let rendered: Vec<String> = findings.iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered[1],
            format!(
                "untrashed container {container_id}: the commit removed it and stands, and \
                 Storage would not move its object to the trash (Storage refused access: \
                 these credentials may write but not delete); the object is still in Storage, \
                 and any later run may trash it"
            ),
        );
        assert!(
            rendered[2].starts_with("checkpoint not written ("),
            "{}",
            rendered[2],
        );
        assert!(
            rendered[2].ends_with(
                "): the commit stands, and the next qualifying commit writes the checkpoint"
            ),
            "{}",
            rendered[2],
        );
        assert!(
            rendered[2].contains("the provider is unavailable"),
            "the line carries what stopped it: {}",
            rendered[2],
        );
    }

    // A freeze commits the same way a sync does, so what its commit could not
    // finish reaches the same findings (spec: OC-6, CK-8).
    #[test]
    fn a_freeze_says_what_its_commit_left_unfinished() {
        let container_id = ContainerId::from_bytes([8; ContainerId::BYTE_LEN]);
        let outcome = FreezeOutcome {
            packs: Vec::new(),
            absorbed: vec![container_id],
            packed_already: 0,
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            commit: Some(unfinished_commit(container_id)),
            degraded: None,
        };

        let findings = Findings::from(&outcome);
        let rendered: Vec<String> = findings.iter().map(ToString::to_string).collect();
        assert_eq!(rendered.len(), 2, "{rendered:?}");
        assert!(
            rendered[0].starts_with(&format!("untrashed container {container_id}: ")),
            "{}",
            rendered[0],
        );
        assert!(
            rendered[1].starts_with("checkpoint not written ("),
            "{}",
            rendered[1],
        );
        assert!(!findings.needs_attention());
    }

    // KL-7 is a loss at the Container level, and the fetch reports it at both
    // levels for that reason: one explicit key-lost marker leaves every Entry
    // the Container holds unreadable.
    #[test]
    fn a_fetch_reports_a_key_lost_container_as_well_as_its_entries() {
        let container_id = ContainerId::from_bytes([7; ContainerId::BYTE_LEN]);
        let outcome = FetchOutcome {
            fetched: Vec::new(),
            containers: Vec::new(),
            skipped: 0,
            mappings: 1,
            surfaced: vec![Declined::KeyLost {
                path: entry_path("albums/lost.jpg"),
                container_id,
            }],
            refused: Vec::new(),
            key_lost: vec![container_id],
            degraded: None,
        };

        let rendered: Vec<String> = Findings::from(&outcome)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            rendered,
            [
                "surfaced albums/lost.jpg: the Library records no key for the Container holding \
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
            fetched: vec![entry_path("albums/spring.jpg")],
            containers: Vec::new(),
            skipped: 0,
            mappings: 1,
            surfaced: vec![Declined::UnreachablePlace {
                path: entry_path("link/authorized_keys"),
                stopped_at: PathBuf::from("/home/someone/mapped/link"),
            }],
            refused: Vec::new(),
            key_lost: Vec::new(),
            degraded: None,
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

    // EP-13's refusal reaches whoever asked for the run the way EP-12's
    // unavailable root does: once for the mapping rather than once per Entry,
    // naming the folder to go and look at, which of this device's mappings
    // stands at it, and the gesture that decides which folder it is. The Entry
    // the refused root says nothing about was placed and is not a finding.
    #[test]
    fn a_refused_root_is_a_finding_that_names_the_mapping_and_the_gesture() {
        let outcome = FetchOutcome {
            fetched: vec![entry_path("albums/spring.jpg")],
            containers: Vec::new(),
            skipped: 0,
            mappings: 1,
            surfaced: Vec::new(),
            refused: vec![RefusedRoot {
                prefix: Some(entry_path("albums")),
                local_root: PathBuf::from("/mnt/copied"),
                reason: RootRefused::MarkerMismatch,
            }],
            key_lost: Vec::new(),
            degraded: None,
        };

        let findings = Findings::from(&outcome);
        assert!(findings.needs_attention());

        let rendered: Vec<String> = findings.iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered,
            [
                "refused root /mnt/copied, which this device maps \"albums\" into: .coffret/root \
                 in it names another identity, so this is not the folder the mapping was \
                 recorded against; nothing was placed into it, and `coffret map` records that \
                 mapping again — with `--reset-marker` where the identity is meant to change"
                    .to_owned()
            ]
        );
    }

    // The same finding for a mapping that stands for the whole Library, which
    // EP-9 admits and which has no component to be named by. What EP-13 asks to
    // be named is the mapping and not the folder, so it is named in the only
    // words there are for that one — the sentence a device that maps the root is
    // the only device ever to read.
    #[test]
    fn a_refused_library_root_names_that_mapping_in_the_sentence() {
        let finding = Finding::RefusedRoot {
            prefix: None,
            local_root: PathBuf::from("/mnt/copied"),
            reason: RootRefused::MarkerMismatch,
        };

        let said = finding.to_string();
        assert!(
            said.starts_with(
                "refused root /mnt/copied, which this device maps the Library root into: "
            ),
            "the sentence names the mapping with the only name it has: {said}",
        );
    }

    // A refusal that carries the marker's own answer says that answer here:
    // neither a finding nor a refusal is an error type, so this line is the
    // whole of what the person who asked for the run reads, and without the
    // defect they learn the marker was rejected and never why (spec: EP-13).
    #[test]
    fn a_malformed_marker_says_what_is_wrong_with_it_in_the_sentence() {
        let defects: [(_, &[&str]); 3] = [
            (
                root_marker::parse(&[b'0'; root_marker::MAX_LEN + 1])
                    .expect_err("more bytes than a marker may hold names no identity"),
                &["a marker holds at most"],
            ),
            (
                root_marker::parse(&[0xff, 0xfe]).expect_err("those bytes are not text"),
                &["a marker's content is text and this is not"],
            ),
            (
                root_marker::parse(b"not an identity").expect_err("that text names no identity"),
                // The defect that is a wrapper of its own. Its first line says
                // no more than the refusal it stands under already said, so
                // the line has to carry the reading beneath it as well: that
                // is where a person meets how a root's identity is spelled.
                &[
                    "a marker's content is the spelling of an identity and this is not",
                    "lowercase hexadecimal characters a root's identity is spelled as",
                ],
            ),
        ];
        for (cause, wanted) in defects {
            let finding = Finding::RefusedRoot {
                prefix: Some(entry_path("albums")),
                local_root: PathBuf::from("/mnt/copied"),
                reason: RootRefused::MarkerMalformed { cause },
            };

            let said = finding.to_string();
            assert!(
                said.contains(".coffret/root in it names no identity"),
                "{said}",
            );
            for defect in wanted {
                assert!(said.contains(defect), "{said}");
            }
        }
    }

    // One Entry that was placed is the whole answer: there is nothing for the
    // caller to act on.
    #[test]
    fn one_entry_placed_reports_nothing() {
        let of = |fetch| Findings::from(&EntryFetchOutcome::of(fetch));
        assert!(of(EntryFetch::Placed).is_empty());
        assert!(of(EntryFetch::AlreadyPresent).is_empty());
        assert_eq!(
            of(EntryFetch::Surfaced(Declined::ForeignFile {
                path: entry_path("albums/theirs.jpg"),
            }))
            .len(),
            1
        );
    }

    /// A read that stepped over positions of the committed set, as each run's
    /// outcome carries it.
    fn degraded_keyring(lost: u16, unfetched: u16) -> DegradedKeyring {
        DegradedKeyring::new(Generation::FIRST, 3, lost, unfetched)
    }

    // A fetch, one Entry's fetch and a freeze that committed nothing each tell
    // whoever ran them that the committed Keyring is short, and none of them
    // turns the exit status over it: the reads went on, and the next run that
    // commits repairs the set (spec: KL-5, KL-15, RV-2).
    #[test]
    fn a_degraded_keyring_is_reported_by_every_run_that_read_it_and_needs_no_attention() {
        let fetch = FetchOutcome {
            fetched: vec![entry_path("albums/kept.jpg")],
            containers: Vec::new(),
            skipped: 0,
            mappings: 1,
            surfaced: Vec::new(),
            refused: Vec::new(),
            key_lost: Vec::new(),
            degraded: Some(degraded_keyring(1, 0)),
        };
        let entry = EntryFetchOutcome {
            fetch: EntryFetch::Placed,
            degraded: Some(degraded_keyring(1, 0)),
        };
        let freeze = FreezeOutcome {
            packs: Vec::new(),
            absorbed: Vec::new(),
            packed_already: 2,
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            commit: None,
            degraded: Some(degraded_keyring(1, 0)),
        };

        for findings in [
            Findings::from(&fetch),
            Findings::from(&entry),
            Findings::from(&freeze),
        ] {
            let found: Vec<&Finding> = findings.iter().collect();
            assert!(
                matches!(
                    found[..],
                    [Finding::DegradedKeyring {
                        lost: 1,
                        unfetched: 0,
                        replicas: 3,
                        ..
                    }]
                ),
                "{found:?}",
            );
            assert!(!findings.needs_attention(), "{found:?}");
        }
    }

    // Loss is said as loss only where the read established it; a set whose
    // replicas Storage merely did not hand over is not called degraded
    // (spec: KL-15).
    #[test]
    fn a_degraded_keyring_is_said_as_loss_only_where_a_loss_is_established() {
        let said = |lost, unfetched| {
            Findings::from(&EntryFetchOutcome {
                fetch: EntryFetch::Placed,
                degraded: Some(degraded_keyring(lost, unfetched)),
            })
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .concat()
        };

        let lost = said(1, 0);
        assert!(lost.starts_with("degraded keyring: "), "{lost}");
        assert!(lost.contains("at least 1 of the 3 replicas"), "{lost}");
        assert!(
            lost.contains(&format!("generation {} is", Generation::FIRST.get())),
            "{lost}",
        );

        let both = said(1, 1);
        assert!(both.starts_with("degraded keyring: "), "{both}");
        assert!(
            both.contains("and Storage did not hand over 1 more"),
            "{both}",
        );

        let unfetched = said(0, 2);
        assert!(!unfetched.contains("degraded keyring"), "{unfetched}");
        assert!(
            unfetched.contains("Storage did not hand over 2 of the 3 replicas")
                && unfetched.contains("is not established"),
            "{unfetched}",
        );
    }

    /// A repair of the first generation that put back `positions`.
    fn repair(positions: Vec<u16>) -> KeyringRepair {
        KeyringRepair {
            generation: Generation::FIRST,
            rewritten: RewrittenReplicas::assembled(positions)
                .expect("a repair puts back at least one position"),
        }
    }

    // KL-15: a repair a run performed is never silent. A sync and a freeze
    // that committed say each one, after what they left alone, and neither
    // turns the verdict over it: the set is whole again.
    #[test]
    fn a_repair_a_commit_performed_is_reported_last_and_needs_no_attention() {
        let repaired = || CommitOutcome {
            repairs: vec![repair(vec![0]), repair(vec![1, 2])],
            ..unfinished_commit(ContainerId::from_bytes([3; ContainerId::BYTE_LEN]))
        };
        let sync = SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 0,
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            settled: Vec::new(),
            commit: Some(repaired()),
        };
        let freeze = FreezeOutcome {
            packs: Vec::new(),
            absorbed: Vec::new(),
            packed_already: 0,
            mappings: 1,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            commit: Some(repaired()),
            degraded: None,
        };

        for findings in [Findings::from(&sync), Findings::from(&freeze)] {
            let found: Vec<&Finding> = findings.iter().collect();
            assert!(
                matches!(
                    found[..],
                    [
                        Finding::UntrashedRemoval { .. },
                        Finding::CheckpointFailed { .. },
                        Finding::KeyringRepaired { rewritten: one, .. },
                        Finding::KeyringRepaired { rewritten: two, .. },
                    ] if one.get() == 1 && two.get() == 2
                ),
                "{found:?}",
            );
            assert!(!findings.needs_attention(), "{found:?}");
        }
    }

    // A run whose commit failed after a repair still says the repair, found
    // wherever in the error's chain the commit's failure is; a failure that
    // carries none, or no commit failure at all, says nothing (spec: KL-15).
    #[test]
    fn a_run_that_failed_after_a_repair_reports_the_repair() {
        let failed = |repairs| {
            crate::Error::from(SyncError::Commit(CommitFailure {
                error: Box::new(CommitError::ConflictLimitReached { attempts: 8 }),
                repairs,
            }))
        };

        let said: Vec<String> = Findings::repaired_before(&failed(vec![repair(vec![1])]))
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            said,
            [format!(
                "repaired the Keyring: 1 replica of generation {} was missing or unreadable, \
                 and was rewritten from a surviving one",
                Generation::FIRST.get(),
            )],
        );
        assert!(Findings::repaired_before(&failed(Vec::new())).is_empty());
        assert!(Findings::repaired_before(&crate::Error::NoStateDirectory).is_empty());
    }

    // The sentence counts in the singular and the plural, and names the
    // generation; nothing else of the repair is said (spec: KL-15, EL-1).
    #[test]
    fn a_repair_is_said_in_the_singular_and_the_plural() {
        assert_eq!(
            Finding::from(&repair(vec![2])).to_string(),
            format!(
                "repaired the Keyring: 1 replica of generation {} was missing or unreadable, \
                 and was rewritten from a surviving one",
                Generation::FIRST.get(),
            ),
        );
        assert_eq!(
            Finding::from(&repair(vec![0, 1, 2])).to_string(),
            format!(
                "repaired the Keyring: 3 replicas of generation {} were missing or \
                 unreadable, and were rewritten from a surviving one",
                Generation::FIRST.get(),
            ),
        );
    }
}
