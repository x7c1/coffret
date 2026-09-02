use crate::folder::Folder;
use crate::noted::Noted;
use crate::reported::Reported;

use super::FreezeStatus;

/// What one freeze of one folder has come to.
///
/// The server's own state and not the Library's: it is about work in flight on
/// this device, it is gone when the process is, and none of it is ever uploaded.
/// What the Library holds afterwards is the listing's to say, as it is for a
/// sync — this says what a run of the flow did and what it left alone.
///
/// The folder is here, unlike a sync's account of itself, because a freeze is of
/// one folder (spec: PK-17): the status bar names the book being brought in, and
/// the retry needs the same name to be offered under.
///
/// What is *not* here is a count of how far along the run is. A freeze commits
/// one batch — the Packs are built, uploaded and committed together (spec: PK-7,
/// CP-1) — so there is no per-file moment for the flow to report, and a count
/// that moved in between would be this server inventing progress the flow does
/// not have. What is stated is what came of it.
#[derive(Clone, Debug)]
pub struct FreezeActivity {
    /// The folder being packed.
    pub folder: Folder,
    /// Where the freeze stands.
    pub status: FreezeStatus,
    /// How many Packs the run built, and `0` until it is over.
    pub packs: usize,
    /// How many Entries those Packs hold, and `0` until it is over.
    ///
    /// Beside [`packs`](Self::packs) rather than instead of it, because the two
    /// together are the whole of what a person wanted: their book went up as a
    /// handful of objects rather than as one per page.
    pub entries: usize,
    /// What the run found and did not act on (spec: PK-14, EP-12).
    pub noted: Vec<Noted>,
    /// The refusal that stopped the freeze, where one did.
    ///
    /// One refusal and not one per file: what stops a freeze is Storage being
    /// unreachable or this device's own catalog or disk refusing, and one batch
    /// either commits or does not.
    pub stopped: Option<Reported>,
}

impl FreezeActivity {
    /// A freeze that has been armed and has packed nothing yet.
    pub(super) fn starting(folder: Folder) -> Self {
        Self {
            folder,
            status: FreezeStatus::Freezing,
            packs: 0,
            entries: 0,
            noted: Vec::new(),
            stopped: None,
        }
    }
}
