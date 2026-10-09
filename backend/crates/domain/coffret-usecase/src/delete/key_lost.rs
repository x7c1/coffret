use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKeyStatus, KeyTable};

use crate::commit::{catch_up, read_committed, CommitPolicy};
use crate::delete::delete_error::DeleteResult;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Which Containers the committed Keyring maps to a key-lost marker, read the
/// way a deletion reads it (spec: KL-1, KL-7).
///
/// The one fact [`preview_delete`](super::preview_delete) cannot take from the
/// catalog, fetched for a caller that wants the preview to say which Packs the
/// run would be refused for rather than learn it from the run. It is the run's
/// own first step and nothing more: the catalog is caught up to the Library's
/// head (spec: CK-9), and the committed Keyring that head names is read. So the
/// plan the preview then works out over the catalog is the plan a run started
/// now would work out — they can differ only where the Library moves in
/// between.
///
/// It writes nothing to the Library. The catch-up does write this device's
/// catalog, which is what every flow does before it reads one, and is why this
/// is not free: it takes Storage and the Library's keys, where the preview
/// alone takes neither.
pub async fn committed_key_lost(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &LibraryKeys,
    policy: &CommitPolicy,
) -> DeleteResult<BTreeSet<ContainerId>> {
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    // A Library that has committed nothing has no Keyring, and no Container to
    // have lost the key of (spec: FM-13).
    let Some(checkpoint) = index.checkpoint().await? else {
        return Ok(BTreeSet::new());
    };
    let read = read_committed(
        store,
        keys.control(),
        &policy.retry,
        &caught.listing,
        checkpoint.keyring(),
    )
    .await?;
    Ok(lost(&read.key_table))
}

/// Which Containers of a key table carry a key-lost marker (spec: KL-7).
pub(super) fn lost(key_table: &KeyTable) -> BTreeSet<ContainerId> {
    key_table
        .elements()
        .iter()
        .filter(|element| element.key == ContainerKeyStatus::KeyLost)
        .map(|element| element.container_id)
        .collect()
}
