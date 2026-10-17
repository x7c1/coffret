use std::collections::BTreeMap;
use std::ops::Range;

use coffret_format::{ContainerOutline, Parcels};
use coffret_model::{ContainerId, EntryMetadata, EntryNumber};

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
        // By entry number, which is how the catalog and the table name the
        // same Entry: the table records the path each was captured under, and
        // a rename gives an Entry a current path the table never records
        // (spec: FM-9, FM-15).
        let mut covered: BTreeMap<EntryNumber, (EntryMetadata, Range<u64>)> = BTreeMap::new();
        for (number, entry) in outline.numbered_entries() {
            if number == asked.target.location.entry_number {
                continue;
            }
            let range = parcels.overlapping(outline, &entry.extent)?;
            if range
                .clone()
                .all(|index| asked.parcels.contains(&index) || held.contains_key(&index))
            {
                covered.insert(number, (entry.clone(), range));
            }
        }
        if covered.is_empty() {
            return Ok(Vec::new());
        }

        // Where the catalog holds each covered Entry now. One the catalog does
        // not hold in this Container is in another one now, or nowhere: not a
        // companion.
        let mut targets = Vec::new();
        for location in self.index.entries_in(container_id).await? {
            if !covered.contains_key(&location.entry_number) {
                continue;
            }
            match translate::target_of(self.index, location.path()).await {
                Ok(target) if target.location == location => targets.push(target),
                // The catalog changed between the two questions: not a
                // companion of this read.
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
                let (entry, parcels) = covered.remove(&target.location.entry_number)?;
                Some(Member {
                    target,
                    entry,
                    parcels,
                })
            })
            .collect())
    }
}
