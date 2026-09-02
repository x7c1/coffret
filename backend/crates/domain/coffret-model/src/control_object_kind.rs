/// Which kind of control state a Storage Object carries.
///
/// Control objects hold the Library's own bookkeeping — never user data, which
/// travels in Containers. Each kind is encrypted under its own purpose key, so
/// a future kind arrives as a new variant together with a new info string.
///
/// The kind is what an object *is*, and it rides in the authenticated header
/// (FM-11). What an object is *for* — a link in the control-head chain, a
/// checkpoint, a Keyring replica — is what its name says (FM-12), and the two
/// are not the same question: the head chain admits two kinds under one name
/// form, because whichever of them wins a head's commit slot takes that head's
/// successor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlObjectKind {
    /// A Journal record: one committed step of the commit protocol.
    Journal,
    /// A Keyring replica: the mapping from every current Container to its Key
    /// Envelope, or to a key-lost marker where no reachable envelope opens it.
    Keyring,
    /// An ordinary Index Snapshot: the Library state a reader starts from,
    /// checkpointing the head it represents.
    IndexSnapshot,
    /// An activation Index Snapshot: the checkpoint that activates a new Master
    /// Key epoch by winning a head's commit slot.
    ///
    /// It carries the same checkpoint content an ordinary Index Snapshot does,
    /// plus the fields activation needs, but it is a kind of its own so that a
    /// misfiled or renamed object — an ordinary Snapshot presented as a head, or
    /// a head presented as an ordinary checkpoint — is refused on the plaintext
    /// header and by the purpose key, before any payload is read (FM-11, FM-12).
    ActivationSnapshot,
}

impl ControlObjectKind {
    /// Every kind this format version defines (FM-11).
    ///
    /// For the callers that have to visit all of them: asking which kinds a name
    /// admits, sizing what one of each may cost, covering the set in a test. One
    /// list, so that a future kind is one edit here rather than a hunt for the
    /// copies that were left behind.
    pub const ALL: [Self; 4] = [
        Self::Journal,
        Self::Keyring,
        Self::IndexSnapshot,
        Self::ActivationSnapshot,
    ];
}
