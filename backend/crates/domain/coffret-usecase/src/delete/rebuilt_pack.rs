use coffret_model::ContainerId;

/// One Pack a deletion replaced, and the replacement it committed
/// (spec: PK-10, PK-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuiltPack {
    /// The Pack the batch removed (spec: CP-14).
    pub replaced: ContainerId,
    /// The replacement the batch added, under a new Container ID.
    pub container_id: ContainerId,
    /// How many Entries the replacement carries forward.
    pub kept: usize,
    /// How many Entries it omits — the named ones.
    pub omitted: usize,
    /// How many bytes were read: the old Pack, whole.
    pub read: u64,
    /// How many bytes the replacement weighs on Storage.
    pub written: u64,
}
