use coffret_device::{EntryPath, Finding, FindingReason, RootRefused, RootUnavailable};

use crate::api_error::refused_root_said;

/// One thing a run that succeeded still has to say.
///
/// One shape for the sync and for the freeze alike, because the obligation is
/// one obligation and the person who dropped the file does not know which flow
/// met it: both hand back [`Finding`]s, and a second vocabulary would be a
/// second set of sentences for the same states.
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
/// and no path at all. The Entry Path is another matter: it is the user's own
/// name for their own file, it is what the row on the screen is keyed by, and
/// the listing carries it already.
#[derive(Clone, Debug)]
pub struct Noted {
    /// The Entry this is about, and `None` where it is about no single Entry.
    pub path: Option<String>,
    /// One sentence a person could read.
    pub message: String,
}

impl Noted {
    /// What a run reported, as the browser is told it, with the record it
    /// already made of itself left out.
    pub(crate) fn of(finding: &Finding) -> Option<Self> {
        match finding {
            Finding::Surfaced { path, reason } => Some(Self {
                path: Some(path.as_str().to_owned()),
                message: said(reason).to_owned(),
            }),
            // `path` stays `None` for the reason a refused root's does, and the
            // mapping the finding names stays out of the message: the terminal
            // says which of a device's mappings it was, and an Entry Path
            // component does not cross this boundary any more than the folder
            // does (spec: EL-1).
            Finding::UnavailableRoot { reason, .. } => Some(Self {
                path: None,
                message: unavailable(*reason).to_owned(),
            }),
            // `path` stays `None`: the finding is about a mapping of this
            // device and not about any one Entry under it.
            Finding::RefusedRoot { prefix, reason, .. } => Some(Self {
                path: None,
                message: refused(prefix.as_ref(), reason),
            }),
            Finding::LockedContainer { .. } => Some(Self {
                path: None,
                message: "the Library records no key for one of the Containers this run met"
                    .to_owned(),
            }),
            // Reported because the run already did what there was to do about it,
            // which is exactly why it is not shown: it leaves nothing for the
            // person who dropped a file.
            Finding::Settled(_) => None,
        }
    }
}

/// The sentence an unavailable root is put in front of a person as.
///
/// The two states are said apart rather than folded into one, because they are
/// the two an unavailable root is made of (spec: EP-12) and only one of them is
/// a folder that could not be read:
/// a root that is there and empty on a filesystem the mapping does not record
/// reads perfectly well, and calling it unreadable would send somebody looking
/// for a permission problem instead of a disk that is not plugged in. What they
/// share is the consequence, which is why each sentence ends in it — nothing
/// under such a root was walked, so a run carrying one has covered less than
/// this device's mappings do.
///
/// Only one of them ends in a gesture, for the reason the device layer's own
/// sentences do: a root that is not there is a disk to plug in or a share to
/// mount, and it comes back on its own. A root that is empty on a filesystem the
/// mapping does not record does not — every later run reports it again until
/// somebody records the mapping afresh, which is the same gesture
/// [`refused_root_said`] ends on, and a person told only what is wrong has no
/// way of knowing that.
fn unavailable(reason: RootUnavailable) -> &'static str {
    match reason {
        RootUnavailable::Missing => {
            "a folder this device maps is not there, so nothing in it was looked at"
        }
        RootUnavailable::AnotherFilesystem => {
            "a folder this device maps is empty and stands on another filesystem, so nothing in \
             it was looked at; where it really is empty, record that mapping again with `coffret \
             map`"
        }
    }
}

/// The sentence a refused root is put in front of a person as (spec: EP-13).
///
/// One sentence for all seven cases, unlike the unavailable root above, and for
/// the reason the device layer gives: what a person does about every one of them
/// is the same gesture, and it is a gesture at a terminal rather than in the
/// browser. Which case it was is the terminal's to spell out — this is one line
/// beside one row, and the folder stays out of it the way an unavailable root's
/// does.
///
/// The sentence itself is [`refused_root_said`]'s, shared with the refusal a
/// request that met the same state is answered with: a person meets this folder
/// through a fill and through a click on a file in it, and reading two accounts
/// of one mapping would leave them looking for two problems. It names the
/// mapping, which is why what is shared is a function rather than one fixed
/// string. The whole set is still matched rather than defaulted, so a case
/// EP-13 grows is one this stops compiling over.
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use coffret_device::Reconciled;
    use coffret_model::ContainerId;

    use super::*;
    use crate::entry_paths::entry_path;

    const PREFIX: &str = "albums";
    const LOCAL_ROOT: &str = "/mnt/photos";

    // The boundary the arm above draws, pinned: the sentence here is the static
    // one about a folder that was not looked at, so neither the prefix nor the
    // folder crosses (spec: EL-1).
    #[test]
    fn an_unavailable_root_carries_neither_the_mapping_nor_the_folder() {
        let noted = Noted::of(&Finding::UnavailableRoot {
            prefix: Some(entry_path(PREFIX)),
            local_root: PathBuf::from(LOCAL_ROOT),
            reason: RootUnavailable::AnotherFilesystem,
        })
        .expect("an unavailable root is something to say");

        assert_eq!(
            noted.path, None,
            "the finding is about a mapping of this device and not about one Entry",
        );
        assert!(
            !noted.message.contains(PREFIX),
            "which of the device's mappings it was is the terminal's to name: {}",
            noted.message,
        );
        assert!(
            !noted.message.contains(LOCAL_ROOT),
            "and the folder never crosses this boundary at all: {}",
            noted.message,
        );
    }

    // The other finding about a mapped root, for the same folder: it names the
    // mapping and still leaves the local path out, which is where the line
    // actually falls (spec: EL-1, EP-13).
    #[test]
    fn a_refused_root_names_the_mapping_and_still_leaves_the_folder_out() {
        let noted = Noted::of(&Finding::RefusedRoot {
            prefix: Some(entry_path(PREFIX)),
            local_root: PathBuf::from(LOCAL_ROOT),
            reason: RootRefused::MarkerMismatch,
        })
        .expect("a refused root is something to say");

        assert_eq!(noted.path, None);
        assert!(noted.message.contains(PREFIX), "{}", noted.message);
        assert!(!noted.message.contains(LOCAL_ROOT), "{}", noted.message);
    }

    #[test]
    fn the_two_shapes_of_an_unavailable_root_are_said_apart() {
        let said = |reason| {
            Noted::of(&Finding::UnavailableRoot {
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
            said(RootUnavailable::AnotherFilesystem).contains("record that mapping again"),
            "the state a run repeats forever says what settles it, the way a refused root does",
        );
        assert!(
            !said(RootUnavailable::Missing).contains("record that mapping again"),
            "and a root that is not there is one to reconnect rather than one to record",
        );
    }

    // A batch this run settled is the one finding nobody has to act on, so it
    // is the one that reaches no screen (spec: OC-7).
    #[test]
    fn a_settled_batch_is_not_shown() {
        let settled = Finding::Settled(Reconciled::Completed {
            container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
            entries: 2,
        });

        assert!(Noted::of(&settled).is_none());
    }
}
