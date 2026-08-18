/// Which kind of control state a Storage Object carries.
///
/// Control objects hold the Library's own bookkeeping — never user data, which
/// travels in Containers. Each kind is encrypted under its own purpose key, so
/// a future kind arrives as a new variant together with a new info string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlObjectKind {
    /// A Journal record: one committed step of the commit protocol.
    Journal,
    /// A Keyring replica: the mapping from every current Container to its Key
    /// Envelope, or to a key-lost marker where no reachable envelope opens it.
    Keyring,
    /// An Index Snapshot: the Library state a reader starts from.
    IndexSnapshot,
}
