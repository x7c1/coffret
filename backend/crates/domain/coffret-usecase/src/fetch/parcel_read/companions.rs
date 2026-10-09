use std::collections::BTreeMap;
use std::ops::Range;

use coffret_format::{ContainerOutline, Parcels};
use coffret_model::{ContainerId, EntryMetadata, EntryPath};

use crate::device_state::HeldParcel;
use crate::fetch::fetch_error::FetchResult;
use crate::fetch::parcel_read::{never_placed, Member, ParcelRead};
use crate::fetch::{select, translate};

impl ParcelRead<'_> {
    /// The other Entries of the Container that the parcels this read will have
    /// wholly cover, and that this device would place (spec: PK-16, EP-10,
    /// EP-11).
    ///
    /// The parcels it will have are the ones it asks for and the ones the
    /// device holds. Each Entry is translated the way any fetch translates one
    /// (spec: EP-9) and put through the same selection, so one that is not
    /// mapped, is already here, or has anything in its way is not written —
    /// and is not reported either, since nobody asked for it.
    pub(super) async fn companions(
        &self,
        outline: &ContainerOutline,
        parcels: &Parcels,
        container_id: ContainerId,
        asked: &Member,
        held: &BTreeMap<u64, HeldParcel>,
    ) -> FetchResult<Vec<Member>> {
        let mut covered: BTreeMap<EntryPath, (EntryMetadata, Range<u64>)> = BTreeMap::new();
        for entry in outline.entries() {
            if &entry.path == asked.target.path() {
                continue;
            }
            let range = parcels.overlapping(outline, &entry.extent)?;
            if range
                .clone()
                .all(|index| asked.parcels.contains(&index) || held.contains_key(&index))
            {
                covered.insert(entry.path.clone(), (entry.clone(), range));
            }
        }

        let mut targets = Vec::new();
        for path in covered.keys() {
            match translate::target_of(self.index, path).await {
                Ok(target) if target.location.container_id == container_id => targets.push(target),
                // In another Container now, or nowhere this device would put
                // it: not a companion.
                Ok(_) => {}
                Err(error) if never_placed(&error) => {}
                Err(error) => return Err(error),
            }
        }
        let selection = select::select(self.index, self.reading.destinations, targets).await?;

        Ok(selection
            .wanted
            .into_iter()
            .filter_map(|target| {
                let (entry, parcels) = covered.remove(target.path())?;
                Some(Member {
                    target,
                    entry,
                    parcels,
                })
            })
            .collect())
    }
}
