use coffret_model::{ControlObjectName, Generation};

use crate::commit_slot::CommitSlot;

/// A commit slot a batch's Journal create was about to consume, as this device
/// recorded it before the create could reach Storage (spec: OC-2).
///
/// It is what makes the second proof of OC-3 possible: a batch whose commit
/// outcome is unknown is proven not to have committed once every slot it
/// recorded is held by an authenticated record that does not name its
/// Containers (spec: OC-3). Reading a slot back needs what consuming it needed,
/// and nothing more: the generation, which names the object `head-<generation>`
/// (spec: CP-15), and the identifier Storage minted for it where Storage mints
/// identifiers (spec: CP-2). A Storage that keys objects by name has none, and
/// the name alone is the slot.
///
/// A batch that rebases records one of these for each attempt (spec: CP-4), so
/// a batch can hold several. They belong to the batch rather than to one of its
/// rows, because every row of the batch rode on the same creates. Like the rows
/// themselves they never leave the device (spec: CK-7).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptedSlot {
    generation: Generation,
    provider_id: Option<String>,
}

impl AttemptedSlot {
    /// The slot at `generation`, under the identifier Storage minted for it, or
    /// under its name alone where `provider_id` is `None`.
    pub fn new(generation: Generation, provider_id: Option<String>) -> Self {
        Self {
            generation,
            provider_id,
        }
    }

    /// What a commit into `slot` at `generation` attempted.
    pub(crate) fn consuming(generation: Generation, slot: &CommitSlot) -> Self {
        Self::new(generation, slot.as_provider_id().map(str::to_owned))
    }

    /// The generation the create would have given its record.
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// The identifier Storage minted for the slot, or `None` where the name is
    /// the slot.
    pub fn provider_id(&self) -> Option<&str> {
        self.provider_id.as_deref()
    }

    /// The name the record at this slot is stored under (spec: CP-15).
    pub fn name(&self) -> ControlObjectName {
        ControlObjectName::head(self.generation)
    }

    /// The slot itself, for reading back what it holds.
    pub(crate) fn commit_slot(&self) -> CommitSlot {
        let name = self.name().to_string();
        match &self.provider_id {
            Some(id) => CommitSlot::provider_id(name, id),
            None => CommitSlot::by_name(name),
        }
    }
}
