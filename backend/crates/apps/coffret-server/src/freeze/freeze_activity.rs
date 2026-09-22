use coffret_device::Step;

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
/// How far along the run is *is* here, in [`step`](Self::step), and it is the
/// flow's own answer rather than this server's. A freeze commits one batch — the
/// Packs are built, uploaded and committed together (spec: PK-7, CP-1) — so
/// there is no per-file outcome to report until it is over; what there is, and
/// what a person watching several hundred pages go up needs, is which phase the
/// run is in and how much of that phase is done, which the flow reports through
/// the same port the command line draws its progress line from. The counts below
/// are still `0` until the batch commits, because those are outcomes.
#[derive(Clone, Debug)]
pub struct FreezeActivity {
    /// Which run of the freeze this is, counted from the start of this process.
    ///
    /// What a screen tells one run's account of itself from the next's: a line
    /// somebody has read and put away must not take the next book's line with
    /// it. Stamped on by [`Freezes`](super::Freezes) rather than carried here
    /// from the flow, so a fresh one is `0` until it is published.
    pub run: u64,
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
    /// How far into the run the flow has got, and `None` before it has said and
    /// once it is over.
    ///
    /// The same [`Step`](coffret_device::Step) the command line renders, from
    /// the same port and meaning the same thing — a phase of the flow, and how
    /// many units of it are done — so that a browser and a terminal watching one
    /// Library cannot disagree about where a run has got to.
    pub step: Option<Step>,
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
            run: 0,
            folder,
            status: FreezeStatus::Freezing,
            packs: 0,
            entries: 0,
            noted: Vec::new(),
            step: None,
            stopped: None,
        }
    }
}
