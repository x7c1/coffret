use std::collections::BTreeMap;

use coffret_model::{ContainerAddition, EntryNumber, KeyEnvelope};

use crate::entry_ref::EntryRef;

/// One Container of a batch, ready to be committed.
///
/// The two halves travel together and end up in two different objects, which is
/// the whole reason this type exists. What the Container holds goes into the
/// Journal record (spec: CP-11); the key that opens it goes into the Keyring
/// candidate and never into the record (spec: CP-11, KL-7). A caller that held
/// them apart could commit a record naming a Container the committed Keyring has
/// no envelope for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAddition {
    /// What the Journal record records about the Container (spec: CP-11).
    pub addition: ContainerAddition,
    /// The envelope the next Keyring generation maps it to (spec: KL-7).
    pub envelope: KeyEnvelope,
    /// Which current Entry each of its Entries takes the place of, by its entry
    /// number here, where it takes the place of one: the Entry a sync's
    /// replacement updates, the one-file Entries a freeze's Pack absorbs, the
    /// kept Entries a rebuilt Pack carries forward (spec: PK-7, PK-10).
    ///
    /// A new file takes the place of nothing and is not here. What this says is
    /// what a rebase follows: an Entry another writer renamed since the batch
    /// was prepared is taken over at the path it stands at now, rather than
    /// brought back at the one it was prepared under (spec: EP-7).
    pub(crate) supersedes: BTreeMap<EntryNumber, EntryRef>,
}

impl PreparedAddition {
    /// Pairs what a record says about a Container with the key that opens it.
    pub const fn new(addition: ContainerAddition, envelope: KeyEnvelope) -> Self {
        Self {
            addition,
            envelope,
            supersedes: BTreeMap::new(),
        }
    }

    /// The same addition, saying which current Entry each of its Entries takes
    /// the place of.
    pub(crate) fn superseding(mut self, supersedes: BTreeMap<EntryNumber, EntryRef>) -> Self {
        self.supersedes = supersedes;
        self
    }
}
