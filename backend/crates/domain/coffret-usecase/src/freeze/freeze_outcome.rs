use coffret_model::ContainerId;

use crate::commit::CommitOutcome;
use crate::freeze::frozen_pack::FrozenPack;
use crate::freeze::not_frozen::NotFrozen;
use crate::unavailable_root::UnavailableRoot;

/// What one freeze built, absorbed, and left alone.
///
/// Two halves, and the second is the one that matters most. [`commit`] says what
/// became of the Library, and a run that selected nothing carries `None` there
/// rather than an empty commit — a Journal record for a batch that changes
/// nothing is a generation spent on nothing (spec: CP-1). [`surfaced`] says what
/// the run could not absorb and why, and it is not an afterthought: a scan
/// selecting freeze candidates has to surface every file that needs an update,
/// so a caller reads this list rather than assuming that a successful freeze
/// means every local file is packed and current (spec: PK-14). [`unavailable`]
/// carries the same obligation for a whole mapping rather than for one file.
///
/// [`commit`]: Self::commit
/// [`surfaced`]: Self::surfaced
/// [`unavailable`]: Self::unavailable
#[derive(Debug)]
pub struct FreezeOutcome {
    /// The Packs this run built and committed, in the order it built them —
    /// which is the Entry Path order of the Entries they hold (spec: PK-3).
    pub packs: Vec<FrozenPack>,
    /// The one-file Containers those Packs absorbed, which the batch removed
    /// (spec: PK-7, CP-14).
    ///
    /// A newly imported file has no removal, and an existing Pack never appears
    /// here: a freeze neither reads nor rewrites one (spec: PK-1, PK-2).
    pub absorbed: Vec<ContainerId>,
    /// How many of the local files this run considered were already held by a
    /// Pack whose Entry the file still matches — the files within this device's
    /// scope (spec: EP-10) and under the folder the request named (spec: PK-17).
    ///
    /// Nothing to do, and nothing wrong: `freeze` persists no folder state, so a
    /// second run over the same folder simply finds every file already packed
    /// (spec: PK-2).
    pub packed_already: usize,
    /// What the run found and could not absorb (spec: PK-14).
    pub surfaced: Vec<NotFrozen>,
    /// The mappings whose local roots the device could not vouch for
    /// (spec: EP-12).
    ///
    /// A freeze infers no deletion, so the only harm an unavailable root does it
    /// is silence — and silence is the one outcome PK-14 forbids, which is why
    /// the mapping is reported rather than the run refused. It matters here
    /// because a run that packed nothing because a disk is unplugged is
    /// otherwise indistinguishable from the ordinary second run over an
    /// already-packed folder: same empty [`packs`](Self::packs), same empty
    /// [`absorbed`](Self::absorbed), same `None` [`commit`](Self::commit). This
    /// field is what tells the two apart. Nothing else about the run changes: an
    /// unavailable root contributes no candidate, so it can neither absorb nor
    /// remove anything.
    ///
    /// Unlike every other field here, this one is not bounded by the folder the
    /// request named (spec: PK-17): it names every mapping the device holds whose
    /// root the run could not vouch for, including one standing for a subtree
    /// outside that folder. The prefix bounds which *files* a run considers, and
    /// a root it could not open is not a file it passed over — a run narrowed to
    /// one folder still says which of the device's roots were not there to walk.
    pub unavailable: Vec<UnavailableRoot>,
    /// What the commit did, or `None` when the run had nothing to commit.
    pub commit: Option<CommitOutcome>,
}

impl FreezeOutcome {
    /// How many Entries the run packed.
    pub fn frozen_entries(&self) -> usize {
        self.packs.iter().map(|pack| pack.entries).sum()
    }
}
