use coffret_model::ContainerId;

use crate::commit::{CommitOutcome, ShortKeyring};

/// What a repack did.
///
/// A run that selected nothing, or whose every selected Pack was already the
/// cut it would make, commits nothing and is an ordinary success: `removed` and
/// `added` are empty and `commit` is `None` (spec: PK-23, CP-1).
#[derive(Debug)]
pub struct RepackOutcome {
    /// Every current Pack holding at least one Entry under the folder, which
    /// took part whole (spec: PK-23).
    pub selected: Vec<ContainerId>,
    /// The selected Packs that were already a segment of the cut, left exactly
    /// as they were.
    pub kept: Vec<ContainerId>,
    /// The selected Packs the batch replaced, now in the provider's trash
    /// unless the commit's outcome says one would not go (spec: OC-6).
    pub removed: Vec<ContainerId>,
    /// The Packs the batch added, in Entry Path order.
    pub added: Vec<RepackedPack>,
    /// What the run committed, or `None` where it had nothing to rewrite.
    ///
    /// Carries every Keyring repair the commit performed, and the removals
    /// Storage would not trash (spec: KL-15, OC-6).
    pub commit: Option<CommitOutcome>,
    /// The degradation the run's read of the committed Keyring found and no
    /// commit spoke for (spec: KL-15).
    pub short: Option<ShortKeyring>,
}

impl RepackOutcome {
    /// How many bytes the run sent to Storage: the new Packs' objects.
    pub fn uploaded(&self) -> u64 {
        self.added.iter().map(|pack| pack.written).sum()
    }
}

/// One Pack a repack added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepackedPack {
    /// Its new Container ID (spec: CP-14).
    pub container_id: ContainerId,
    /// How many Entries it carries.
    pub entries: usize,
    /// What it measures before padding, which the target is held against
    /// (spec: PK-6).
    pub footprint: u64,
    /// How many bytes its object weighs on Storage.
    pub written: u64,
}
