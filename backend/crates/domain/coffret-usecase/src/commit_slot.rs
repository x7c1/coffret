use crate::error::{Error, Result};

/// A reservation for exactly one conditional create, bound to one name.
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
/// Either way the name travels with the reservation, because on a name-keyed
/// store the name *is* what the create is conditional on. A slot that carried
/// only a minted identifier would let two writers spend one reservation under
/// two names and both succeed there — which is exactly the hole that made the
/// head chain non-exclusive while its two successor kinds were named
/// differently.
///
/// A slot is otherwise opaque to callers: they reserve one, spend it on a
/// [`put_if_absent`](crate::ObjectStore::put_if_absent), and either commit or
/// lose the race. Spending the same slot twice is what raises
/// [`Error::AlreadyExists`], and a slot reserved from one store means nothing
/// to another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitSlot {
    name: String,
    provider_id: Option<String>,
}

impl CommitSlot {
    /// Reserves the slot a store keys by object name.
    ///
    /// There is nothing to allocate: the name the create passes is the slot, so
    /// the reservation carries no identifier of its own.
    pub fn by_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            provider_id: None,
        }
    }

    /// Reserves the identifier a store minted for the object to come, together
    /// with the name that create will give it.
    pub fn provider_id(name: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            provider_id: Some(id.into()),
        }
    }

    /// The name the object created here will be stored under.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The minted identifier, or `None` when the object's name is the slot.
    pub fn as_provider_id(&self) -> Option<&str> {
        self.provider_id.as_deref()
    }

    /// The name this slot stands for, for a store whose exclusion is on the
    /// name.
    ///
    /// A slot carrying a minted identifier was reserved from a store that mints
    /// them, and a name-keyed store cannot honour it: creating under the name
    /// alone would be exclusive against the wrong thing.
    pub fn require_name(&self) -> Result<&str> {
        match self.as_provider_id() {
            Some(id) => Err(Error::Unsupported {
                detail: format!(
                    "a slot reserved as minted id {id:?} cannot be spent \
                     on a store that keys objects by name"
                ),
            }),
            None => Ok(&self.name),
        }
    }

    /// The minted identifier this slot stands for, for a store whose exclusion
    /// is on the identifier.
    ///
    /// A slot carrying no identifier was reserved from a name-keyed store, and
    /// a store that mints them has nothing to create under.
    pub fn require_provider_id(&self) -> Result<&str> {
        self.as_provider_id().ok_or_else(|| Error::Unsupported {
            detail: format!(
                "a slot reserved for the name {:?} cannot be spent \
                 on a store that mints identifiers",
                self.name
            ),
        })
    }
}
