use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKeyStatus, KeyTable};

/// Which Containers of a key table carry a key-lost marker (spec: KL-7).
///
/// The one fact a deletion's plan needs that the catalog does not hold: the run
/// reads it off the committed Keyring after its catch-up, where
/// [`preview_delete`](super::preview_delete) reads no Keyring and counts only
/// what its caller names.
pub(super) fn lost(key_table: &KeyTable) -> BTreeSet<ContainerId> {
    key_table
        .elements()
        .iter()
        .filter(|element| element.key == ContainerKeyStatus::KeyLost)
        .map(|element| element.container_id)
        .collect()
}
