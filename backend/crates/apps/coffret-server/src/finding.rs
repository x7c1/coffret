use coffret_device::{EntryPath, FindingReason, RootRefused, RootUnavailable};

use crate::api_error::refused_root_said;

/// One thing a run that succeeded still has to say.
///
/// One shape for the sync and for the freeze alike, because the obligation is
/// one obligation and the person who dropped the file does not know which flow
/// met it: both hand back [`coffret_device::Finding`]s, and a second vocabulary
/// would be a second set of sentences for the same states.
///
/// The same word as the device layer's, because it is the same thing — what a
/// run reports about one file or one mapping without failing — told to a
/// browser rather than to a terminal.
///
/// A run that returns `Ok` has not necessarily backed everything up: a file
/// whose Entry lives in a Pack is left byte-for-byte as it is, a file this device
/// had and no longer has is reported rather than deleted from the Library, and a
/// mapped folder the device could not vouch for was not walked at all
/// (spec: PK-14, EP-10, EP-12). Reading only the counts would tell somebody their
/// file is safe when it is not, which is the one outcome those rules forbid.
///
/// So each of them reaches the browser, and the person who dropped a file is told
/// without opening a terminal. What a run *settled* on the way is not among them:
/// a batch an interrupted run left behind and this one finished is said for the
/// record, and there is nobody to say it to here.
///
/// # What is left out, deliberately
///
/// The local path. A finding about a mapped root names the folder on this device,
/// and a local path is not something to put across this boundary or into a
/// diagnostic event — so an unavailable root arrives as the sentence about it
/// and no path at all. The Entry Path is another matter: this is a response to
/// the browser asking what the runs found and not a record of them (spec:
/// EL-1), it is what the row on the screen is keyed by, and the listing carries
/// it already. A finding waits here in memory until that request arrives, and
/// is written nowhere that outlives the process.
#[derive(Clone, Debug)]
pub struct Finding {
    /// The Entry this is about, and `None` where it is about no single Entry.
    pub path: Option<String>,
    /// A user-facing explanation.
    pub message: String,
    /// Which way the run left this alone, for a page to branch on rather than
    /// to read out of the sentence: `surfaced` or `key_lost` for one Entry,
    /// `root_missing`, `root_on_another_filesystem` or `refused_root` for a
    /// mapping, `key_lost` for a Container, `keyring_degraded` or
    /// `keyring_repaired` for the Library's committed Keyring (spec: KL-5,
    /// KL-15), and `parcel_unheld` for a kept parcel read again (spec: PK-21).
    ///
    /// A refusal's `reason` vocabulary, spelled as a refusal spells it, because
    /// the states are the same ones: one Entry whose Container the Library
    /// records no key for is `key_lost` whether a fetch declined it or a run
    /// reported it, and a mapped folder that is not the one its mapping was
    /// recorded against is `refused_root` either way. The five a refusal never
    /// carries are a run's own — a mapped root it could not vouch for is not
    /// something a request is declined over (spec: EP-12), a degraded Keyring
    /// is read through rather than refused (spec: RV-2), a repaired one is
    /// work a run did (spec: KL-13), and a kept parcel not held was read again
    /// from Storage rather than refused over (spec: PK-21). The whole set is
    /// named here for the reason a refusal's is: a page writes a branch per
    /// reason, and one it has never heard of is one it falls off the end of.
    pub reason: &'static str,
    /// The finding about one Entry, by the name the device layer gives it —
    /// `ForeignFile`, `KeyLost` and the rest — and `None` for a finding about a
    /// mapping or a Container.
    ///
    /// The refusal's `surfaced` field, paired with `reason` the way a refusal
    /// pairs them: `KeyLost` beside `key_lost`, every other name beside
    /// `surfaced`. A page that reads a declined fetch already reads these, and
    /// reads a finding with the same branches.
    pub surfaced: Option<&'static str>,
}

impl Finding {
    /// Every finding of a run's that a browser is told, in the order the run
    /// reported them.
    pub(crate) fn all_of(findings: &coffret_device::Findings) -> Vec<Self> {
        findings.iter().filter_map(Self::of).collect()
    }

    /// What a run reported, as the browser is told it, with the record it
    /// already made of itself left out.
    pub(crate) fn of(finding: &coffret_device::Finding) -> Option<Self> {
        use coffret_device::Finding;

        match finding {
            Finding::Surfaced { path, reason } => {
                let (reason_named, surfaced) = named(reason);
                Some(Self {
                    path: Some(path.as_str().to_owned()),
                    message: said(reason).to_owned(),
                    reason: reason_named,
                    surfaced: Some(surfaced),
                })
            }
            // `path` stays `None` for the reason a refused root's does, and the
            // mapping the finding names stays out of the message: the terminal
            // says which of a device's mappings it was, and an Entry Path
            // component does not cross this boundary any more than the folder
            // does (spec: EL-1).
            Finding::UnavailableRoot { reason, .. } => Some(Self {
                path: None,
                message: unavailable(*reason).to_owned(),
                reason: match reason {
                    RootUnavailable::Missing => "root_missing",
                    RootUnavailable::AnotherFilesystem => "root_on_another_filesystem",
                },
                surfaced: None,
            }),
            // `path` stays `None`: the finding is about a mapping of this
            // device and not about any one Entry under it. One reason for all
            // seven cases of EP-13, as the refusal a request meets has: the
            // guidance is the same for each, and which case it was is the
            // terminal's to say.
            Finding::RefusedRoot { prefix, reason, .. } => Some(Self {
                path: None,
                message: refused(prefix.as_ref(), reason),
                reason: "refused_root",
                surfaced: None,
            }),
            Finding::KeyLostContainer { .. } => Some(Self {
                path: None,
                message: "the Library records no key for one of the Containers this run met"
                    .to_owned(),
                reason: "key_lost",
                surfaced: None,
            }),
            // Shown although nobody has to act on it, unlike the three at the
            // end of this match: a person who only uses the explorer is
            // exactly who KL-15 says must hear of replica loss, and nothing
            // else they do would tell them. No path, because it is about the
            // Library and not about one Entry, and no counts or generation in
            // the sentence, for the reason a mapping stays out of the ones
            // above: the terminal is where the particulars are said.
            Finding::DegradedKeyring { lost, .. } => Some(Self {
                path: None,
                message: degraded_keyring(*lost > 0).to_owned(),
                reason: "keyring_degraded",
                surfaced: None,
            }),
            // Shown for the reason a degraded one is, and in the words the
            // terminal says it in: KL-15 says a repair performed is never
            // silent, and one sentence for both shells means a person who met
            // it in one reads the same news in the other. Unlike a degraded
            // Keyring's, the sentence names the generation and counts the
            // replicas put back — both numbers, neither a path nor a name, so
            // nothing of the person's crosses here (spec: EL-1).
            Finding::KeyringRepaired { .. } => Some(Self {
                path: None,
                message: finding.to_string(),
                reason: "keyring_repaired",
                surfaced: None,
            }),
            // Shown although nothing is left to do, for the reason a degraded
            // Keyring is: PK-21 says it is said, the second read is one the
            // provider observes, and a person who only opens files hears of a
            // disk that changed under the device nowhere else. No path, and
            // neither which Container nor which parcel, because none is the
            // person's to act on and a Container's ID is not something to put
            // across this boundary (spec: EL-1); how many there were is the
            // number of these a run carries.
            Finding::UnheldParcel { .. } => Some(Self {
                path: None,
                message: UNHELD_PARCEL.to_owned(),
                reason: "parcel_unheld",
                surfaced: None,
            }),
            // Not shown, because none of these leaves anything for the person
            // who dropped a file. Each leaves the committed state correct: a
            // settled batch is one the run already dealt with, and what a
            // refused disposal, an untrashed removal or a failed checkpoint
            // leaves is a later run's to finish, or orphan cleanup's to find
            // (spec: OC-1, OC-4, OC-6, CK-8). Named one by one, so a finding
            // the device grows has to be placed here rather than falling
            // through.
            Finding::Settled(_)
            | Finding::UntrashedRemoval { .. }
            | Finding::CheckpointFailed { .. } => None,
        }
    }
}

/// The sentence shown for a kept parcel that was not held after all
/// (spec: PK-21).
///
/// In the Library's words rather than the format's: a parcel is a part of a
/// Container, which a person meets in the explorer already (`key_lost`), and
/// what the person needs to know is that something kept on this device went
/// bad and was read from Storage again, as the terminal says it — not which
/// Container, nor where on the disk (spec: EL-1).
const UNHELD_PARCEL: &str =
    "a part of a Container kept on this device was gone or damaged and was read from Storage again";

/// The explanation shown for a committed Keyring a run's read had to step over
/// a position of (spec: KL-5, KL-15).
///
/// Said as loss only where the run established one. A replica Storage merely
/// did not hand over may read back perfectly well next time, and telling
/// somebody their Library has lost part of its Keyring on that evidence would
/// be reporting a loss a transient fault made up. Either way the files still
/// open (spec: RV-2), and the next run that writes to the Library deals with it
/// before it commits (spec: KL-13, KL-16).
fn degraded_keyring(established: bool) -> &'static str {
    if established {
        "the Library's Keyring is degraded: some of its replicas are missing or unreadable. \
         Files still open, and the next run that writes to the Library repairs it"
    } else {
        "Storage did not hand over some replicas of the Library's Keyring, so whether any of \
         them is lost is not established. Files still open, and the next run that writes to the \
         Library examines it"
    }
}

/// The explanation shown for an unavailable root.
///
/// The two states are said apart rather than folded into one, because they are
/// the two an unavailable root is made of (spec: EP-12) and only one of them is
/// a folder that could not be read:
/// a root that is there and empty on a filesystem the mapping does not record
/// reads perfectly well, and calling it unreadable would send somebody looking
/// for a permission problem instead of a disk that is not plugged in. What they
/// share is the consequence, which is why each explanation includes it — nothing
/// under such a root was walked, so a run carrying one has covered less than
/// this device's mappings do.
///
/// Only the latter needs more than the ordinary reconnect explanation: a root
/// that is not there is a disk to plug in or a share to mount, and it comes back
/// on its own. A root that is empty on another filesystem needs the intended
/// filesystem reconnected, unless its owner deliberately means to map that
/// empty folder in its place; the command needed for that choice is made
/// reachable here as it is in [`refused_root_said`].
fn unavailable(reason: RootUnavailable) -> &'static str {
    match reason {
        RootUnavailable::Missing => {
            "a folder this device maps is not there, so nothing in it was looked at"
        }
        RootUnavailable::AnotherFilesystem => {
            "a folder this device maps is empty and stands on another filesystem, so nothing in \
             it was looked at. Reconnect the intended filesystem. If this empty folder is \
             deliberately taking its place, open a terminal on the device serving the Library. \
             Run `coffret mappings --library <library>` to inspect the recorded mappings and \
             `coffret map --help` to see how to record it again. Then return to the explorer and \
             try the action again"
        }
    }
}

/// The guidance shown for a refused root (spec: EP-13).
///
/// Shared guidance for all seven cases distinguishes reconnecting the intended
/// folder, re-recording the same mapping after confirming its folder, and
/// deliberately mapping another folder in its place. The terminal spells out
/// which case it was; the browser keeps the local folder out of the message.
///
/// The guidance itself is [`refused_root_said`]'s, shared with the refusal a
/// request that met the same state is answered with: a person meets this folder
/// through a fill, through a click on a file in it, and through a drop into it,
/// and reading two accounts of one mapping would leave them looking for two
/// problems. It names the mapping, which is why what is shared is a function
/// rather than one fixed string. The whole set is still matched rather than
/// defaulted, so a case EP-13 grows is one this stops compiling over.
fn refused(prefix: Option<&EntryPath>, reason: &RootRefused) -> String {
    match reason {
        RootRefused::NoExpectedIdentity
        | RootRefused::ManagementAreaMissing
        | RootRefused::ManagementAreaNotADirectory
        | RootRefused::MarkerMissing
        | RootRefused::MarkerNotARegularFile
        | RootRefused::MarkerMalformed { .. }
        | RootRefused::MarkerMismatch => refused_root_said(prefix),
    }
}

/// The sentence one reason is put in front of a person as.
///
/// Written here rather than taken from
/// [`FindingReason`](coffret_device::FindingReason)'s own `Display`, for the
/// reason every message on these routes is written here: the device layer's
/// wording is for whoever is keeping the Library, at a terminal, with the run's
/// whole output in front of them. This is one line beside one row.
///
/// The whole set is matched rather than defaulted, so a reason the device layer
/// grows is one this stops compiling over instead of quietly showing under
/// somebody else's words.
///
/// A reason that names a folder on this device says it at a terminal and not
/// here, which is why this takes the reason by reference and still answers in
/// static sentences: a local path is one of the things this boundary leaves out,
/// exactly as an unavailable root's folder is left out above.
fn said(reason: &FindingReason) -> &'static str {
    match reason {
        FindingReason::ChangedInPack => {
            "this file changed, and what it changed from is inside a Pack — coffret cannot \
             replace it yet"
        }
        FindingReason::DeletedLocally => {
            "this device had this file and it is gone; the Library still holds it"
        }
        FindingReason::MovedToTrash => {
            "this file was deleted from the Library, and this device's unedited copy was moved \
             to the trash"
        }
        FindingReason::KeptEdited => {
            "this file was deleted from the Library, and this device's copy is kept because it \
             changed; move or remove it to stop this report"
        }
        FindingReason::MoveToTrashRefused { .. } => {
            "this file was deleted from the Library, and moving this device's copy to the trash \
             was refused; it is left in place, and the next sync tries again"
        }
        FindingReason::KeyLost => "the Library records no key for the Container holding this file",
        FindingReason::ForeignFile => {
            "a file this device did not put there stands where this Entry belongs"
        }
        FindingReason::LocallyChanged => "what this device wrote there has since changed or gone",
        FindingReason::WitnessedDeletion => "this device witnessed this file's deletion",
        FindingReason::UnreachablePlace { .. } => {
            "a folder on the way to this file is not a folder of this device's mapped folder"
        }
        FindingReason::ReservedComponent => {
            "this file's path carries `.coffret`, which is coffret's own folder inside a mapped \
             folder and never a place a file is put"
        }
    }
}

/// The `reason` and the `surfaced` name one per-file finding goes on the wire
/// under, beside the sentence [`said`] gives it.
///
/// Paired as a declined fetch pairs them
/// ([`ApiError::declined`](crate::api_error::ApiError::declined)): a lost key is
/// `key_lost`, because it is the one finding nothing about this device
/// remedies, and every other is `surfaced`. The names are the device layer's
/// variant names, which is what a refusal's `surfaced` carries. Matched in full
/// for the reason `said` is: a reason the device layer grows is one this stops
/// compiling over rather than one that reaches a page unnamed.
fn named(reason: &FindingReason) -> (&'static str, &'static str) {
    match reason {
        FindingReason::KeyLost => ("key_lost", "KeyLost"),
        FindingReason::ForeignFile => ("surfaced", "ForeignFile"),
        FindingReason::LocallyChanged => ("surfaced", "LocallyChanged"),
        FindingReason::WitnessedDeletion => ("surfaced", "WitnessedDeletion"),
        FindingReason::UnreachablePlace { .. } => ("surfaced", "UnreachablePlace"),
        FindingReason::ReservedComponent => ("surfaced", "ReservedComponent"),
        // The two a sync finds and a fetch never declines over, so they reach
        // a page only on a finding — under the same reason as the others, and
        // not as `pack_resident`: that one is an upload refused before anything
        // was written, where this is a file already on the disk that the run
        // left as it was.
        FindingReason::ChangedInPack => ("surfaced", "ChangedInPack"),
        FindingReason::DeletedLocally => ("surfaced", "DeletedLocally"),
        FindingReason::MovedToTrash => ("surfaced", "MovedToTrash"),
        FindingReason::KeptEdited => ("surfaced", "KeptEdited"),
        FindingReason::MoveToTrashRefused { .. } => ("surfaced", "MoveToTrashRefused"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::num::NonZeroUsize;
    use std::path::{Path, PathBuf};

    use coffret_device::{Disposal, Generation, Settled, StorageError, UnheldReason};
    use coffret_model::ContainerId;

    use super::*;
    use crate::api_error::ApiError;
    use crate::entry_paths::entry_path;

    const PREFIX: &str = "albums";
    const LOCAL_ROOT: &str = "/mnt/photos";

    // The boundary the arm above draws, pinned: the sentence here is the static
    // one about a folder that was not looked at, so neither the prefix nor the
    // folder crosses (spec: EL-1).
    #[test]
    fn an_unavailable_root_carries_neither_the_mapping_nor_the_folder() {
        let finding = Finding::of(&coffret_device::Finding::UnavailableRoot {
            prefix: Some(entry_path(PREFIX)),
            local_root: PathBuf::from(LOCAL_ROOT),
            reason: RootUnavailable::AnotherFilesystem,
        })
        .expect("an unavailable root is something to say");

        assert_eq!(
            finding.path, None,
            "the finding is about a mapping of this device and not about one Entry",
        );
        assert!(
            !finding.message.contains(PREFIX),
            "which of the device's mappings it was is the terminal's to name: {}",
            finding.message,
        );
        assert!(
            !finding.message.contains(LOCAL_ROOT),
            "and the folder never crosses this boundary at all: {}",
            finding.message,
        );
    }

    // The other finding about a mapped root, for the same folder: it names the
    // mapping and still leaves the local path out, which is where the line
    // actually falls (spec: EL-1, EP-13).
    #[test]
    fn refused_root_findings_share_the_request_recovery_for_both_mappings() {
        for (prefix, named) in [
            (Some(entry_path(PREFIX)), PREFIX),
            (None, "the Library root"),
        ] {
            let finding = Finding::of(&coffret_device::Finding::RefusedRoot {
                prefix: prefix.clone(),
                local_root: PathBuf::from(LOCAL_ROOT),
                reason: RootRefused::MarkerMismatch,
            })
            .expect("a refused root is something to say");

            assert_eq!(finding.path, None);
            assert!(finding.message.contains(named), "{}", finding.message);
            assert!(!finding.message.contains(LOCAL_ROOT), "{}", finding.message);
            assert_eq!(
                finding.message,
                refused_root_said(prefix.as_ref()),
                "a background finding and request refusal say the same recovery",
            );
            assert!(
                finding
                    .message
                    .contains("coffret mappings --library <library>")
                    && finding.message.contains("coffret map --help")
                    && finding.message.contains("return to the explorer"),
                "the shared recovery is reachable from the explorer: {}",
                finding.message,
            );
        }
    }

    #[test]
    fn the_two_shapes_of_an_unavailable_root_are_said_apart() {
        let said = |reason| {
            Finding::of(&coffret_device::Finding::UnavailableRoot {
                prefix: None,
                local_root: PathBuf::from(LOCAL_ROOT),
                reason,
            })
            .expect("an unavailable root is something to say")
            .message
        };

        assert_ne!(
            said(RootUnavailable::Missing),
            said(RootUnavailable::AnotherFilesystem),
        );
        assert!(
            said(RootUnavailable::AnotherFilesystem)
                .contains("If this empty folder is deliberately taking its place"),
            "the state a run repeats forever says when recording a mapping remedies it",
        );
        assert!(
            said(RootUnavailable::AnotherFilesystem)
                .contains("return to the explorer and try the action again"),
            "the adjacent mapping recovery says how to resume after either repair",
        );
        assert!(
            !said(RootUnavailable::Missing).contains("record that mapping again"),
            "and a root that is not there is one to reconnect rather than one to record",
        );
    }

    // A batch this run settled is a finding nobody has to act on, so it
    // reaches no screen (spec: OC-7) — however it was settled, a disposal
    // Storage refused among them, since the explorer never said "disposed of"
    // either.
    #[test]
    fn a_settled_batch_is_not_shown() {
        let container_id = ContainerId::from_bytes([9; ContainerId::BYTE_LEN]);
        let refusal = || StorageError::PermissionDenied {
            detail: "these credentials may write but not delete".to_owned(),
            source: None,
        };
        for settled in [
            Settled::Completed {
                container_id,
                entries: 2,
            },
            Settled::Disposed {
                container_id,
                disposal: Disposal::NeverUploaded,
            },
            Settled::Disposed {
                container_id,
                disposal: Disposal::Trashed,
            },
            Settled::Disposed {
                container_id,
                disposal: Disposal::LeftInStorage { cause: refusal() },
            },
        ] {
            assert!(Finding::of(&coffret_device::Finding::Settled(settled)).is_none());
        }
    }

    // What a commit could not finish after its record leaves the committed
    // state correct and nothing for the person in the explorer to do, so it
    // follows a settled batch off the screen (spec: OC-6, CK-8).
    #[test]
    fn what_a_commit_left_unfinished_is_not_shown() {
        let untrashed = coffret_device::Finding::UntrashedRemoval {
            container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
            cause: StorageError::PermissionDenied {
                detail: "these credentials may write but not delete".to_owned(),
                source: None,
            },
        };
        let checkpoint = coffret_device::Finding::CheckpointFailed {
            cause: "the provider is unavailable".to_owned(),
        };

        assert!(Finding::of(&untrashed).is_none());
        assert!(Finding::of(&checkpoint).is_none());
    }

    // A degraded Keyring reaches the explorer, and is said as loss only where
    // the run established one (spec: KL-15). Neither sentence names the
    // generation or a count: those are the terminal's to say.
    #[test]
    fn a_degraded_keyring_is_shown_and_said_as_loss_only_where_established() {
        let shown = |lost, unfetched| {
            Finding::of(&coffret_device::Finding::DegradedKeyring {
                generation: Generation::FIRST,
                replicas: 3,
                lost,
                unfetched,
            })
            .expect("a degraded Keyring is something to say")
        };

        let lost = shown(1, 1);
        assert_eq!(lost.reason, "keyring_degraded");
        assert_eq!(lost.path, None);
        assert_eq!(lost.surfaced, None);
        assert!(lost.message.contains("is degraded"), "{}", lost.message);

        let unfetched = shown(0, 2);
        assert_eq!(unfetched.reason, "keyring_degraded");
        assert!(
            unfetched.message.contains("is not established")
                && !unfetched.message.contains("is degraded"),
            "{}",
            unfetched.message,
        );
    }

    // A kept parcel read again reaches the explorer as one sentence about a
    // part of a Container, and names neither which Container nor which parcel:
    // neither is the person's to act on, and a Container's ID does not cross
    // this boundary (spec: PK-21, EL-1).
    #[test]
    fn an_unheld_parcel_is_shown_without_its_container() {
        let container_id = ContainerId::from_bytes([9; ContainerId::BYTE_LEN]);
        let shown = Finding::of(&coffret_device::Finding::UnheldParcel {
            container_id,
            parcel: 3,
            reason: UnheldReason::Missing,
        })
        .expect("a parcel read again is something to say");

        assert_eq!(shown.reason, "parcel_unheld");
        assert_eq!(shown.path, None);
        assert_eq!(shown.surfaced, None);
        assert_eq!(shown.message, UNHELD_PARCEL);
        assert!(
            !shown.message.contains(&container_id.to_string()),
            "{}",
            shown.message,
        );
    }

    // A repaired Keyring reaches the explorer in the terminal's own words:
    // the generation and how many replicas were put back, and nothing else
    // (spec: KL-15, EL-1).
    #[test]
    fn a_repaired_keyring_is_shown_in_the_terminal_s_words() {
        let repaired = coffret_device::Finding::KeyringRepaired {
            generation: Generation::FIRST,
            rewritten: NonZeroUsize::new(2).expect("two is not zero"),
        };
        let shown = Finding::of(&repaired).expect("a repair is something to say");

        assert_eq!(shown.reason, "keyring_repaired");
        assert_eq!(shown.path, None);
        assert_eq!(shown.surfaced, None);
        assert_eq!(shown.message, repaired.to_string());
        assert_eq!(
            shown.message,
            format!(
                "repaired the Keyring: 2 replicas of generation {} were missing or unreadable, \
                 and were rewritten from a surviving one",
                Generation::FIRST.get(),
            ),
        );
    }

    /// Where the explorer reads the reasons a finding can carry from, relative
    /// to this crate.
    ///
    /// One committed file for the reason the refusals' finding names are one:
    /// the two sides are built by different toolchains, and a browser bundle has
    /// no way to ask a Rust binary what it can say.
    const FINDING_REASONS: &str =
        "../../../../frontend/packages/gateway/api/src/finding-reasons.json";

    /// Where the explorer reads the `surfaced` names from, which a finding
    /// shares with a refusal.
    const SURFACED_FINDINGS: &str =
        "../../../../frontend/packages/gateway/api/src/surfaced-findings.json";

    /// The next finding after `previous` that reaches a browser, and `None`
    /// past the last of them.
    ///
    /// A walk rather than a list, for the reason the refusals' is: the matches
    /// are exhaustive, so a reason or a finding the device layer grows fails to
    /// compile here until it is given its place in the order, and the walk then
    /// visits it without being told to. The per-file ones come in the order
    /// the shared names file holds them: the six a fetch can also decline over
    /// first, as the refusals' walk has them, and the two only a sync finds
    /// after.
    fn after(previous: Option<&coffret_device::Finding>) -> Option<coffret_device::Finding> {
        use coffret_device::Finding;

        let surfaced = |reason| Finding::Surfaced {
            path: entry_path("albums/a.jpg"),
            reason,
        };
        let unavailable = |reason| Finding::UnavailableRoot {
            prefix: None,
            local_root: PathBuf::from(LOCAL_ROOT),
            reason,
        };
        let Some(previous) = previous else {
            return Some(surfaced(FindingReason::ForeignFile));
        };
        match previous {
            Finding::Surfaced { reason, .. } => Some(match reason {
                FindingReason::ForeignFile => surfaced(FindingReason::LocallyChanged),
                FindingReason::LocallyChanged => surfaced(FindingReason::WitnessedDeletion),
                FindingReason::WitnessedDeletion => surfaced(FindingReason::UnreachablePlace {
                    stopped_at: PathBuf::from(LOCAL_ROOT),
                }),
                FindingReason::UnreachablePlace { .. } => surfaced(FindingReason::KeyLost),
                FindingReason::KeyLost => surfaced(FindingReason::ReservedComponent),
                FindingReason::ReservedComponent => surfaced(FindingReason::ChangedInPack),
                FindingReason::ChangedInPack => surfaced(FindingReason::DeletedLocally),
                FindingReason::DeletedLocally => surfaced(FindingReason::MovedToTrash),
                FindingReason::MovedToTrash => surfaced(FindingReason::KeptEdited),
                FindingReason::KeptEdited => surfaced(FindingReason::MoveToTrashRefused {
                    cause: std::sync::Arc::new(coffret_device::DescentError::Io(
                        coffret_usecase::LocalIoError::new(
                            coffret_usecase::LocalOperation::MovingToTrash,
                            LOCAL_ROOT,
                            std::io::Error::other("no trash on this volume"),
                        ),
                    )),
                }),
                FindingReason::MoveToTrashRefused { .. } => unavailable(RootUnavailable::Missing),
            }),
            Finding::UnavailableRoot { reason, .. } => Some(match reason {
                RootUnavailable::Missing => unavailable(RootUnavailable::AnotherFilesystem),
                RootUnavailable::AnotherFilesystem => Finding::RefusedRoot {
                    prefix: None,
                    local_root: PathBuf::from(LOCAL_ROOT),
                    reason: RootRefused::MarkerMismatch,
                },
            }),
            Finding::RefusedRoot { .. } => Some(Finding::KeyLostContainer {
                container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
            }),
            Finding::KeyLostContainer { .. } => Some(Finding::DegradedKeyring {
                generation: Generation::FIRST,
                replicas: 3,
                lost: 1,
                unfetched: 0,
            }),
            Finding::DegradedKeyring { .. } => Some(Finding::KeyringRepaired {
                generation: Generation::FIRST,
                rewritten: NonZeroUsize::MIN,
            }),
            Finding::KeyringRepaired { .. } => Some(Finding::UnheldParcel {
                container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
                parcel: 1,
                reason: UnheldReason::Missing,
            }),
            Finding::UnheldParcel { .. }
            | Finding::Settled(_)
            | Finding::UntrashedRemoval { .. }
            | Finding::CheckpointFailed { .. } => None,
        }
    }

    /// Every finding the walk visits, as the browser is told it.
    fn every_shown() -> Vec<Finding> {
        let mut shown = Vec::new();
        let mut current = after(None);
        while let Some(finding) = current {
            shown.push(Finding::of(&finding).expect("every finding the walk visits is shown"));
            current = after(Some(&finding));
        }
        shown
    }

    /// Every reason this server can put in a finding's `reason` field, each
    /// once, in the order the walk first meets it.
    fn every_reason() -> Vec<&'static str> {
        let mut reasons = Vec::new();
        for finding in every_shown() {
            if !reasons.contains(&finding.reason) {
                reasons.push(finding.reason);
            }
        }
        reasons
    }

    /// A committed file of the explorer's, read as the array of strings it is.
    fn held(relative: &str) -> Vec<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let held = fs::read_to_string(&path)
            .unwrap_or_else(|cause| panic!("{} must be readable: {cause}", path.display()));
        serde_json::from_str(&held)
            .unwrap_or_else(|cause| panic!("{} must be an array of names: {cause}", path.display()))
    }

    // A page branches on these and not on the sentence, so the names are not
    // written down twice: the file the explorer imports is the one list, and
    // this is what holds the server to it. The file holds strings, so the
    // `FindingReason` union in work.ts is not held to it by any compiler;
    // the message asks for that step too.
    #[test]
    fn the_reasons_file_the_explorer_reads_holds_the_reasons_this_server_sends() {
        assert_eq!(
            held(FINDING_REASONS),
            every_reason(),
            "{FINDING_REASONS} has fallen behind `Finding::of`; write these reasons into it, in \
             this order, and bring the `FindingReason` union in run.ts beside it",
        );
    }

    // The per-file names are the refusals' file and not a list of their own:
    // what a finding can say about one Entry is every name a refusal can, and
    // the two a sync finds that no fetch declines over. So the file holds
    // exactly the names a finding sends — the refusals' own case holds it to
    // theirs — and a name added on one side is one both are held to.
    #[test]
    fn the_surfaced_names_a_finding_sends_are_the_shared_file() {
        let sent: Vec<&str> = every_shown()
            .iter()
            .filter_map(|finding| finding.surfaced)
            .collect();

        assert_eq!(
            held(SURFACED_FINDINGS),
            sent,
            "{SURFACED_FINDINGS} has fallen behind `named`; write these names into it, in \
             this order, and bring the `SurfacedFinding` union in refusal.ts beside it",
        );
    }

    // One state, one word and one spelling: where a request can be declined
    // over the same state a run reports, the finding and the refusal carry the
    // same `reason` and the same `surfaced`, compared value for value rather
    // than against a list written out again here.
    #[test]
    fn a_state_a_refusal_and_a_finding_both_name_is_spelled_alike() {
        use coffret_device::Surfaced;

        let path = entry_path("albums/a.jpg");
        let stopped_at = PathBuf::from(LOCAL_ROOT);
        let container_id = ContainerId::from_bytes([9; ContainerId::BYTE_LEN]);
        let pairs = [
            (
                Surfaced::ForeignFile { path: path.clone() },
                FindingReason::ForeignFile,
            ),
            (
                Surfaced::LocallyChanged { path: path.clone() },
                FindingReason::LocallyChanged,
            ),
            (
                Surfaced::WitnessedDeletion { path: path.clone() },
                FindingReason::WitnessedDeletion,
            ),
            (
                Surfaced::UnreachablePlace {
                    path: path.clone(),
                    stopped_at: stopped_at.clone(),
                },
                FindingReason::UnreachablePlace { stopped_at },
            ),
            (
                Surfaced::KeyLost {
                    path: path.clone(),
                    container_id,
                },
                FindingReason::KeyLost,
            ),
            (
                Surfaced::ReservedComponent { path: path.clone() },
                FindingReason::ReservedComponent,
            ),
        ];
        for (declined, reason) in pairs {
            let refusal = ApiError::declined(&declined);
            let finding = Finding::of(&coffret_device::Finding::Surfaced {
                path: path.clone(),
                reason,
            })
            .expect("a per-file finding is something to say");

            assert_eq!(refusal.reason(), Some(finding.reason), "{declined:?}");
            assert_eq!(refusal.surfaced(), finding.surfaced, "{declined:?}");
        }

        let refusal = ApiError::refused_root(None, &coffret_device::Error::NoStateDirectory);
        let finding = Finding::of(&coffret_device::Finding::RefusedRoot {
            prefix: None,
            local_root: PathBuf::from(LOCAL_ROOT),
            reason: RootRefused::MarkerMismatch,
        })
        .expect("a refused root is something to say");
        assert_eq!(refusal.reason(), Some(finding.reason));
        assert_eq!(refusal.surfaced(), finding.surfaced);
    }
}
