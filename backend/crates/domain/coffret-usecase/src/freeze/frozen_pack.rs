use coffret_model::ContainerId;

/// One Pack a freeze built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenPack {
    /// The Pack's Container ID.
    pub container_id: ContainerId,
    /// How many Entries it holds.
    pub entries: usize,
    /// Its pre-padding footprint, which is what the target was compared against
    /// (spec: PK-6).
    pub footprint: u64,
    /// Whether it is an oversized singleton: one Entry that exceeds the target
    /// by itself, which stays indivisible rather than being split across
    /// Containers (spec: PK-3).
    ///
    /// A form of Pack and not a third Container kind — its
    /// [`ContainerKind`](coffret_model::ContainerKind) is `Pack` like any other
    /// (spec: PK-15).
    pub oversized: bool,
}
