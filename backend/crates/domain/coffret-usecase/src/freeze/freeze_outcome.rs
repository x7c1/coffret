use coffret_model::ContainerId;

use crate::commit::CommitOutcome;
use crate::freeze::frozen_pack::FrozenPack;
use crate::freeze::not_frozen::NotFrozen;

/// What one freeze built, absorbed, and left alone.
///
/// Two halves, and the second is the one that matters most. [`commit`] says what
/// became of the Library, and a run that selected nothing carries `None` there
/// rather than an empty commit — a Journal record for a batch that changes
/// nothing is a generation spent on nothing (spec: CP-1). [`surfaced`] says what
/// the run could not absorb and why, and it is not an afterthought: a scan
/// selecting freeze candidates has to surface every file that needs an update,
/// so a caller reads this list rather than assuming that a successful freeze
/// means every local file is packed and current (spec: PK-14).
///
/// [`commit`]: Self::commit
/// [`surfaced`]: Self::surfaced
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
    /// How many Entries under the prefix a Pack already holds and the local file
    /// still matches.
    ///
    /// Nothing to do, and nothing wrong: `freeze` persists no folder state, so a
    /// second run over the same folder simply finds every file already packed
    /// (spec: PK-2).
    pub packed_already: usize,
    /// What the run found and could not absorb (spec: PK-14).
    pub surfaced: Vec<NotFrozen>,
    /// What the commit did, or `None` when the run had nothing to commit.
    pub commit: Option<CommitOutcome>,
}

impl FreezeOutcome {
    /// How many Entries the run packed.
    pub fn frozen_entries(&self) -> usize {
        self.packs.iter().map(|pack| pack.entries).sum()
    }
}
