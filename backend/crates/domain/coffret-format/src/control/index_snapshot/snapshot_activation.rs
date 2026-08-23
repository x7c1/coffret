use coffret_model::Generation;

/// What an activation Index Snapshot carries beyond the checkpoint (MR-2).
///
/// An activation Snapshot wins a head's commit slot instead of a Journal
/// record, which is what atomically fences the writers still on the old epoch
/// (CP-3). These two fields record that act: which head was fenced, and the
/// slot the fence was won at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotActivation {
    /// The generation of the head whose commit slot this activation consumed.
    ///
    /// It is one less than the Snapshot's own generation, which the header
    /// carries (FM-13); it is stated here because the payload has to be able to
    /// disagree with the header for a reader to catch a Snapshot that was moved.
    pub base_head_generation: Generation,
    /// The Storage's own opaque token for that slot, and `None` where the
    /// provider mints none (spec: CP-2, CP-15).
    ///
    /// A name-keyed Storage persists no token at all, so this being absent says
    /// nothing about which kind of Snapshot this is; `base_head_generation` is
    /// what must agree with the header's kind.
    pub activation_slot: Option<String>,
}
