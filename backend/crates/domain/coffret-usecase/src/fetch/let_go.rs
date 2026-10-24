//! Letting go of the parcels this device holds once they have served their
//! purpose (spec: PK-21).
//!
//! Only a held parcel is let go: one whose row says the device keeps it and
//! that the device does keep. A kept parcel that turns out not to be held —
//! gone, or no longer authenticating — is shed where the read meets it,
//! by the same removal of file and row, and never called let go
//! ([`UnheldParcel`](crate::fetch::unheld_parcel::UnheldParcel)).

use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryLocation, EntryNumber};
use tracing::debug;

use crate::device_state::HeldParcel;
use crate::fetch::fetch_error::FetchResult;
use crate::fetch::kept_parcels::KeptParcels;
use crate::fetch::parcel_read::never_placed;
use crate::fetch::translate;
use crate::index::Index;

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
        kept.let_go_parcel(index, parcel).await?;
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
/// A parcel covers the current Entries of the Container it has bytes of, and
/// the catalog says which those are and at which path each stands now: by
/// entry number, so a renamed Entry is waited for at its current path
/// (spec: FM-9, FM-15). An Entry this device does not map is not waited for —
/// a Pack holds whatever folders it was frozen from, and a device mapping only
/// some of them would otherwise hold the rest of the Pack for ever. A
/// Container that has left the current set holds no current Entry, so every
/// parcel of it is let go.
pub(in crate::fetch) async fn let_go_settled(
    index: &dyn Index,
    kept: &KeptParcels<'_>,
    container_id: ContainerId,
) -> FetchResult<()> {
    let held: Vec<HeldParcel> = index
        .held_parcels()
        .await?
        .into_iter()
        .filter(|parcel| parcel.container_id == container_id)
        .collect();
    if held.is_empty() {
        return Ok(());
    }
    let entries = index.entries_in(container_id).await?;
    let mut verdicts: BTreeMap<EntryNumber, bool> = BTreeMap::new();
    let mut let_go_of = 0;
    for parcel in &held {
        let mut all_settled = true;
        for location in entries
            .iter()
            .filter(|location| parcel.covers(location.extent().offset(), location.extent().size()))
        {
            let settled = match verdicts.get(&location.entry_number) {
                Some(settled) => *settled,
                None => {
                    let settled = settled(index, location).await?;
                    verdicts.insert(location.entry_number, settled);
                    settled
                }
            };
            if !settled {
                all_settled = false;
                break;
            }
        }
        if all_settled {
            kept.let_go_parcel(index, parcel).await?;
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
/// that long and nothing else. It asks the catalog for the Entries of each
/// Container parcels are held of, and so it returns at once where nothing is
/// held.
pub async fn let_go_parcels(index: &dyn Index, kept: &KeptParcels<'_>) -> FetchResult<()> {
    let_go_departed(index, kept).await?;
    let held: BTreeSet<ContainerId> = index
        .held_parcels()
        .await?
        .into_iter()
        .map(|parcel| parcel.container_id)
        .collect();
    for container_id in held {
        let_go_settled(index, kept, container_id).await?;
    }
    Ok(())
}

/// Whether one current Entry of a Container holds a parcel of it no longer.
///
/// Settled where no mapping of this device reaches it or none can place it,
/// and where this device holds a row for it at its current path — present, or
/// a deletion it witnessed (spec: EP-9, EP-10, EP-11). Only an Entry this
/// device maps and has never had there is still waited for.
async fn settled(index: &dyn Index, location: &EntryLocation) -> FetchResult<bool> {
    match translate::target_of(index, location.path()).await {
        Ok(_) => {}
        Err(error) if never_placed(&error) => return Ok(true),
        Err(error) => return Err(error),
    }
    Ok(index.local_entry_at(location.path()).await?.is_some())
}
