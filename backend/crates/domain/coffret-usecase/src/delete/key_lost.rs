use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKeyStatus, KeyTable};

use crate::commit::{catch_up, read_committed, CommitPolicy};
use crate::delete::delete_error::DeleteResult;
use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::spool::Spool;
use crate::sync::{complete_after_catch_up, PendingWork};

/// Which Containers the committed Keyring maps to a key-lost marker, read the
/// way a deletion reads it (spec: KL-1, KL-7).
///
/// The one fact [`preview_delete`](super::preview_delete) cannot take from the
/// catalog, fetched for a caller that wants the preview to say which Packs the
/// run would be refused for rather than learn it from the run. It is the run's
/// own first step and nothing more: the catalog is caught up to the Library's
/// head (spec: CK-9), the bookkeeping of a batch of this device's the catch-up
/// found committed is completed through `spool` at `now` (spec: OC-7), and the
/// committed Keyring that head names is read. So the plan the preview then
/// works out over the catalog is the plan a run started now would work out —
/// they can differ only where the Library moves in between.
///
/// It writes nothing to the Library. The catch-up does write this device's
/// catalog, which is what every flow does before it reads one, and is why this
/// is not free: it takes Storage and the Library's keys, where the preview
/// alone takes neither. The completion is what lets the preview count the files
/// such a batch carried in as this device's local copies, as the run would.
pub async fn committed_key_lost(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &LibraryKeys,
    spool: &dyn Spool,
    policy: &CommitPolicy,
    now: DeviceTime,
) -> DeleteResult<BTreeSet<ContainerId>> {
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    // Unowned: a preview holds none of this device's pending work, and leaves
    // the rows to a run that does (spec: OC-2, OC-7).
    complete_after_catch_up(index, spool, now, PendingWork::Unowned).await;
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
