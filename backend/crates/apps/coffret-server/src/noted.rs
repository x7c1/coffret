use coffret_device::{Finding, FindingReason, RootUnavailable};

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
/// and a local path is not something to put across this boundary or into a log
/// line — so an unavailable root arrives as the sentence about it and no path at
/// all. The Entry Path is another matter: it is the user's own name
/// for their own file, it is what the row on the screen is keyed by, and the
/// listing carries it already.
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
            Finding::UnavailableRoot { reason, .. } => Some(Self {
                path: None,
                message: unavailable(*reason).to_owned(),
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
fn unavailable(reason: RootUnavailable) -> &'static str {
    match reason {
        RootUnavailable::Missing => {
            "a folder this device maps is not there, so nothing in it was looked at"
        }
        RootUnavailable::AnotherFilesystem => {
            "a folder this device maps is empty and stands on another filesystem, so nothing in \
             it was looked at"
        }
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
    }
}
