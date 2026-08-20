/// A reservation for exactly one conditional create.
///
/// A control head determines exactly one next commit slot, and the writers that
/// start from that head all aim at it, so the port needs a writer to be able to
/// say "create this object only if nobody else has" and exactly one of them to
/// win (spec: CP-2, CP-3). Providers offer that in two shapes:
///
/// - a store that keys objects by name has nothing to allocate ahead of time,
///   and the name itself is the slot — [`CommitSlot::by_name`];
/// - a store that mints identifiers hands one out first and the create names it
///   — Drive's `files.generateIds` — [`CommitSlot::provider_id`].
///
/// A slot is opaque to callers: they reserve one, spend it on a
/// [`put_if_absent`](crate::ObjectStore::put_if_absent), and either commit or
/// lose the race. Spending the same slot twice is what raises
/// [`Error::AlreadyExists`](crate::Error::AlreadyExists), and a slot reserved
/// from one store means nothing to another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitSlot(Option<String>);

impl CommitSlot {
    /// Reserves the slot a store keys by object name.
    ///
    /// There is nothing to allocate: the name the create passes is the slot, so
    /// the reservation carries no identifier of its own.
    pub const fn by_name() -> Self {
        Self(None)
    }

    /// Reserves the identifier a store minted for the object to come.
    pub fn provider_id(id: impl Into<String>) -> Self {
        Self(Some(id.into()))
    }

    /// The minted identifier, or `None` when the object's name is the slot.
    pub fn as_provider_id(&self) -> Option<&str> {
        self.0.as_deref()
    }
}
