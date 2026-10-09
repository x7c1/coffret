use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use coffret_format::unwrap_container_key;
use coffret_model::{ContainerSummary, KeyEnvelope};
use tracing::{debug, warn};

use crate::device_state::HeldParcel;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::parcel_read::herald::Herald;
use crate::fetch::parcel_read::source::Source;
use crate::fetch::parcel_read::spread::Spread;
use crate::fetch::parcel_read::{front, let_go, Member, ParcelRead, Stroke};
use crate::fetch::target::Target;
use crate::fetch::unheld_parcel::UnheldParcel;

impl ParcelRead<'_> {
    /// Places the Entry `target` stands for, reading the parcels it overlaps
    /// that this device does not hold, and every other Entry those parcels
    /// and the held ones wholly cover.
    ///
    /// The steps, and the rule each answers to:
    ///
    /// 1. **The front** (spec: FM-2, FM-9, PK-16). Header and meta section, the
    ///    same read for every Entry of the Container. The entry table places
    ///    the Entry in the plaintext stream, and the chunk size divides the
    ///    stream into parcels (spec: PK-19) — so which parcels to ask for is
    ///    known before any chunk arrives.
    /// 2. **What comes with it** (spec: PK-16, EP-10, EP-11). Every other Entry
    ///    of the Container lying wholly inside the parcels this read will have —
    ///    the ones it asks for and the ones the device already holds — and that
    ///    this device would place anyway, mapped and with nothing in the way.
    /// 3. **The parcels, in order** (spec: PK-16, PK-21). The stretch holding
    ///    the asked-for Entry first, so its page is up before anything else is
    ///    written, and the caller hears of it the moment it is, before the rest
    ///    of its parcel arrives; then any stretch of held parcels a companion
    ///    lies in. A held parcel is opened from this device; one that is gone
    ///    or will not authenticate is said and asked of Storage again; every
    ///    other parcel is asked of Storage whole, kept, and opened as it
    ///    arrives. Cancellation is asked before each parcel requested from
    ///    Storage and never inside one.
    /// 4. **Let go** (spec: PK-21). Every held parcel of this Container whose
    ///    mapped Entries are all on the device is let go.
    pub(in crate::fetch) async fn read_entry(
        &self,
        summary: &ContainerSummary,
        envelope: &KeyEnvelope,
        target: Target,
    ) -> FetchResult<Stroke> {
        let reading = self.reading;
        let container_id = summary.id;
        let object = summary
            .object_ref
            .as_ref()
            .or_else(|| reading.listing.container(container_id))
            .ok_or(FetchError::ContainerUnreachable { container_id })?;
        let key = unwrap_container_key(reading.keys.container_wrap(), &container_id, envelope)?;
        let object_len = summary.ciphertext_len.get();
        let outline = front::front(reading.store, reading.retry, object, object_len, &key).await?;
        let entry = outline
            .entry_at(target.path())
            .ok_or_else(|| FetchError::EntryMissing {
                container_id,
                path: target.path().clone(),
            })?
            .clone();

        let parcels = outline.parcels(self.kept.len);
        let asked = parcels.overlapping(&outline, &entry.extent)?;
        let mut held: BTreeMap<u64, HeldParcel> = self
            .index
            .held_parcels()
            .await?
            .into_iter()
            .filter(|parcel| parcel.container_id == container_id)
            .map(|parcel| (parcel.index, parcel))
            .collect();

        let asked_path = target.path().clone();
        let mut members = vec![Member {
            target,
            entry,
            parcels: asked.clone(),
        }];
        let companions = self
            .companions(&outline, &parcels, container_id, &members[0], &held)
            .await?;
        members.extend(companions);

        let source = Source {
            store: reading.store,
            retry: reading.retry,
            object,
            object_len,
            outline: &outline,
            key: &key,
            index: self.index,
            now: self.now,
            kept: self.kept,
        };
        let mut stroke = Stroke {
            placed: false,
            alongside: Vec::new(),
            unheld: Vec::new(),
        };
        let herald = Herald::new(self.publication, self.degraded);
        let mut read_from_storage = 0u64;
        let padding_start = outline.plaintext_len() - outline.pad_len();

        for stretch in stretches(&members, &asked) {
            let placements = self.placements(&members, &stretch).await?;
            let start = parcels
                .run(stretch.start)
                .expect("a stretch names parcels the Container has")
                .plaintext()
                .start;
            let mut spread = Spread::new(start, padding_start, placements, &asked_path, &herald);
            let mut cancelled = false;
            for index in stretch.clone() {
                let run = parcels
                    .run(index)
                    .expect("a stretch names parcels the Container has");
                if let Some(parcel) = held.remove(&index) {
                    match source.read_held(&parcel, &run, &mut spread).await {
                        Ok(None) => continue,
                        Ok(Some(reason)) => {
                            // Not held after all: said, let go, and asked of
                            // Storage below like any parcel never read.
                            warn!(
                                container = %container_id,
                                parcel = index,
                                reason = ?reason,
                                "a kept parcel was not held after all and is read again",
                            );
                            herald.unheld(UnheldParcel {
                                container_id,
                                index,
                                reason,
                            });
                            if let Err(error) = let_go::let_go(self.index, self.kept, &parcel).await
                            {
                                spread.abandon();
                                return Err(error);
                            }
                        }
                        Err(error) => {
                            spread.abandon();
                            return Err(error);
                        }
                    }
                }
                if self.cancellation.is_cancelled() {
                    cancelled = true;
                    break;
                }
                if let Err(error) = source.fetch(index, &run, &mut spread).await {
                    spread.abandon();
                    return Err(error);
                }
                read_from_storage += 1;
            }
            let reached = spread.position();
            let published = if cancelled {
                spread.abandon()
            } else {
                spread.finish(self.index, self.now).await?
            };
            for path in published {
                if path == asked_path {
                    stroke.placed = true;
                } else {
                    stroke.alongside.push(path);
                }
            }
            if cancelled {
                debug!(
                    container = %container_id,
                    reached,
                    "a parcel read was cancelled on a parcel boundary",
                );
                break;
            }
        }

        stroke.unheld = herald.into_unheld();
        let_go::let_go_settled(self.index, self.kept, container_id, outline.entries()).await?;
        debug!(
            container = %container_id,
            object = %container_id.object_name(),
            parcels = asked.end - asked.start,
            read_from_storage,
            of = parcels.count(),
            alongside = stroke.alongside.len(),
            placed = stroke.placed,
            "read one Entry out of a Container by its parcels",
        );
        Ok(stroke)
    }
}

/// The stretches of adjacent parcels the members need, the one holding the
/// Entry asked for first and the rest in stream order.
///
/// Each member's parcels are adjacent and lie inside one stretch, because the
/// stretches are the runs of the union of every member's parcels.
fn stretches(members: &[Member], asked: &Range<u64>) -> Vec<Range<u64>> {
    let needed: BTreeSet<u64> = members
        .iter()
        .flat_map(|member| member.parcels.clone())
        .collect();
    let mut stretches: Vec<Range<u64>> = Vec::new();
    for index in needed {
        match stretches.last_mut() {
            Some(last) if last.end == index => last.end += 1,
            _ => stretches.push(index..index + 1),
        }
    }
    if let Some(first) = stretches
        .iter()
        .position(|stretch| stretch.start <= asked.start && asked.end <= stretch.end)
    {
        let holding = stretches.remove(first);
        stretches.insert(0, holding);
    }
    stretches
}
