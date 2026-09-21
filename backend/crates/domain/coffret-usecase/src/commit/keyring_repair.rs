use coffret_model::Generation;

/// One repair a commit performed on the committed Keyring (spec: KL-13,
/// KL-15).
///
/// Every position the commitment declares is examined before a commit writes a
/// generation of its own, and one of these is what that examination leaves
/// behind when it had positions to put back: the generation it was about, and
/// the positions rewritten from a committed valid replica and confirmed by
/// reading back (spec: KL-6, KL-14). An examination that found the set complete
/// leaves none, which is why [`rewritten`](Self::rewritten) is never empty —
/// "nothing needed repairing" is an empty
/// [`CommitOutcome::repairs`](super::CommitOutcome::repairs) rather than a
/// repair that names no position.
///
/// It is on [`CommitOutcome`](super::CommitOutcome) rather than left in a
/// diagnostic event because KL-15 asks for replica loss and the repair
/// performed to reach the user, and an event is a record the person who asked
/// for the run never receives (spec: EL-1). The count is what a shell says out
/// loud; the positions are here because a caller comparing two runs — the same
/// position going missing again and again — is reading about one object rather
/// than about a number.
#[derive(Debug, Clone)]
pub struct KeyringRepair {
    /// The committed generation that was examined and repaired (spec: KL-3).
    pub generation: Generation,
    /// The replica positions this examination rewrote, in ascending order, and
    /// never empty.
    pub rewritten: Vec<u16>,
}
