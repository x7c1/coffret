use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryMetadata, EntryPath};
use tracing::debug;

use crate::device_state::HeldParcel;
use crate::fetch::fetch_error::FetchResult;
use crate::fetch::kept_parcels::KeptParcels;
use crate::fetch::translate;
use crate::index::Index;

/// Lets one held parcel go: its file first, then its row.
///
/// In that order so that the row always covers whatever file there is — a
/// letting go interrupted between the two leaves a row naming a file that is
/// gone, which the next letting go discards again and a read meets as a parcel
/// not held. Both halves succeed on what is already gone (spec: OC-8, PK-21).
pub(in crate::fetch) async fn let_go(
    index: &dyn Index,
    kept: &KeptParcels<'_>,
    parcel: &HeldParcel,
) -> FetchResult<()> {
    kept.files.discard(&parcel.path).await?;
    index
        .let_go_parcel(parcel.container_id, parcel.index)
        .await?;
    Ok(())
}

/// Lets go of every held parcel whose Container has left the current set
/// (spec: PK-21).
///
/// A replaced or removed Container's parcels serve nothing: no current Entry
/// is in them, and a reader asking for one of its Entries asks for it in the
/// Container that now holds it (spec: CP-1). Cheap where nothing is held,
/// which is the ordinary state of a device that has placed what it read.
pub(in crate::fetch) async fn let_go_departed(
    index: &dyn Index,
    kept: &KeptParcels<'_>,
) -> FetchResult<()> {
    let held = index.held_parcels().await?;
    if held.is_empty() {
        return Ok(());
    }
    let current: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let mut let_go_of = 0;
    for parcel in held
        .iter()
        .filter(|parcel| !current.contains(&parcel.container_id))
    {
        let_go(index, kept, parcel).await?;
        let_go_of += 1;
    }
    if let_go_of > 0 {
        debug!(
            parcels = let_go_of,
            "let go of the parcels of Containers that left the current set",
        );
    }
    Ok(())
}

/// Lets go of every held parcel of one Container whose mapped Entries are all
/// on this device or witnessed absent (spec: PK-21, EP-10, EP-11).
///
/// `entries` is what the Container holds, from its own entry table or from the
/// catalog; a parcel covers the ones it has bytes of. An Entry this device does
/// not map is not waited for — a Pack holds whatever folders it was frozen
/// from, and a device mapping only some of them would otherwise hold the rest
/// of the Pack for ever.
pub(in crate::fetch) async fn let_go_settled(
    index: &dyn Index,
    kept: &KeptParcels<'_>,
    container_id: ContainerId,
    entries: &[EntryMetadata],
) -> FetchResult<()> {
    let held: Vec<HeldParcel> = index
        .held_parcels()
        .await?
        .into_iter()
        .filter(|parcel| parcel.container_id == container_id)
        .collect();
    let mut verdicts: BTreeMap<&EntryPath, bool> = BTreeMap::new();
    let mut let_go_of = 0;
    for parcel in &held {
        let mut all_settled = true;
        for entry in entries
            .iter()
            .filter(|entry| parcel.covers(entry.extent.offset(), entry.extent.size()))
        {
            let settled = match verdicts.get(&entry.path) {
                Some(settled) => *settled,
                None => {
                    let settled = settled(index, container_id, &entry.path).await?;
                    verdicts.insert(&entry.path, settled);
                    settled
                }
            };
            if !settled {
                all_settled = false;
                break;
            }
        }
        if all_settled {
            let_go(index, kept, parcel).await?;
            let_go_of += 1;
        }
    }
    if let_go_of > 0 {
        debug!(
            container = %container_id,
            parcels = let_go_of,
            "let go of parcels whose mapped Entries are all on this device",
        );
    }
    Ok(())
}

/// Lets go of every held parcel that has served its purpose: those of
/// Containers that left the current set, and those whose mapped Entries are
/// all on this device or witnessed absent (spec: PK-21).
///
/// What a run that changed the Library's current set without reading any parcel
/// calls afterwards — a deletion this device committed, a catch-up. A sync or a
/// freeze does not call it: a Container either of them replaces keeps its
/// parcels until the next fetch or catch-up lets them go, which costs disk for
/// that long and nothing else. It asks the catalog for the Entries of the
/// Containers parcels are held of, which is a walk of the catalog, and so it
/// returns at once where nothing is held.
pub async fn let_go_parcels(index: &dyn Index, kept: &KeptParcels<'_>) -> FetchResult<()> {
    let_go_departed(index, kept).await?;
    let held: BTreeSet<ContainerId> = index
        .held_parcels()
        .await?
        .into_iter()
        .map(|parcel| parcel.container_id)
        .collect();
    if held.is_empty() {
        return Ok(());
    }
    let mut entries: BTreeMap<ContainerId, Vec<EntryMetadata>> = BTreeMap::new();
    for location in index.entries_under(None).await? {
        if held.contains(&location.container_id) {
            entries
                .entry(location.container_id)
                .or_default()
                .push(location.entry);
        }
    }
    for container_id in held {
        let of_it = entries.remove(&container_id).unwrap_or_default();
        let_go_settled(index, kept, container_id, &of_it).await?;
    }
    Ok(())
}

/// Whether one Entry of a Container holds a parcel of it no longer.
///
/// Settled where the Library no longer holds it in this Container, where no
/// mapping of this device reaches it or none can place it, and where this
/// device holds a row for it — present, or a deletion it witnessed
/// (spec: EP-9, EP-10, EP-11). Only an Entry this device maps and has never had
/// is still waited for.
async fn settled(
    index: &dyn Index,
    container_id: ContainerId,
    path: &EntryPath,
) -> FetchResult<bool> {
    match index.entry_at(path).await? {
        Some(location) if location.container_id == container_id => {}
        _ => return Ok(true),
    }
    match translate::target_of(index, path).await {
        Ok(_) => {}
        Err(error) if super::never_placed(&error) => return Ok(true),
        Err(error) => return Err(error),
    }
    Ok(index.local_entry_at(path).await?.is_some())
}
