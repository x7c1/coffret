use std::ops::Range;

use coffret_model::EntryExtent;

use crate::container_reader::chunk_layout::ChunkLayout;
use crate::container_reader::chunk_run::ChunkRun;
use crate::container_reader::container_outline::ContainerOutline;
use crate::error::Result;
use crate::parcel_len::ParcelLen;

/// One Container's chunk sequence, divided into parcels.
///
/// A parcel is `n` consecutive chunks counted from the first chunk, where
/// `n = max(1, S div chunk size)` and the chunk size is the one this object's
/// header records; the last parcel holds whatever chunks remain (spec: PK-19).
/// Its boundaries are positions in the chunk sequence and have nothing to do
/// with where Entries begin or end, which is the whole point: a read aimed at a
/// parcel tells the Storage provider which parcel was wanted and nothing about
/// the Entry that wanted it (spec: PK-20).
///
/// Everything here follows from the header and the meta section alone
/// (spec: FM-2, FM-9), so a reader names the parcels an Entry needs before any
/// chunk has arrived. The front of the object — its header and meta section —
/// belongs to no parcel and is read on its own (spec: PK-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parcels {
    layout: ChunkLayout,
    /// How many chunks one parcel holds, at least one.
    per_parcel: u64,
}

impl Parcels {
    /// The division of one outline's chunk sequence under a parcel length.
    pub(super) fn of(outline: &ContainerOutline, len: ParcelLen) -> Self {
        let chunk_size = u64::from(outline.chunk_size().get());
        Self {
            layout: outline.layout(),
            per_parcel: (len.get() / chunk_size).max(1),
        }
    }

    /// How many chunks one parcel holds, the `n` of PK-19.
    pub fn chunks_per_parcel(&self) -> u64 {
        self.per_parcel
    }

    /// How many parcels the Container has, at least one: a Container of no
    /// more than `n` chunks is one parcel (spec: PK-19).
    pub fn count(&self) -> u64 {
        self.layout.chunk_count().div_ceil(self.per_parcel)
    }

    /// The chunks of parcel `index`, or `None` where the Container has no such
    /// parcel.
    ///
    /// What a reader hands a Storage range read is this run's
    /// [`ciphertext`](ChunkRun::ciphertext): exactly the messages of the
    /// parcel's chunks, and never anything shorter (spec: PK-16).
    pub fn run(&self, index: u64) -> Option<ChunkRun> {
        if index >= self.count() {
            return None;
        }
        let first = index * self.per_parcel;
        let count = self.per_parcel.min(self.layout.chunk_count() - first);
        Some(ChunkRun::new(self.layout, first, count))
    }

    /// The parcel indexes an Entry's extent overlaps, as a half-open range.
    ///
    /// An Entry spanning a parcel boundary needs every parcel it overlaps, and
    /// one of no bytes still names the parcel holding the chunk it stands at
    /// (spec: PK-16, FM-5). An extent the stream does not reach is refused as
    /// [`chunks_covering`](ContainerOutline::chunks_covering) refuses it.
    pub fn overlapping(
        &self,
        outline: &ContainerOutline,
        extent: &EntryExtent,
    ) -> Result<Range<u64>> {
        let chunks = outline.chunks_covering(extent.range())?;
        let first = chunks.first() / self.per_parcel;
        let last = (chunks.first() + chunks.count() - 1) / self.per_parcel;
        Ok(first..last + 1)
    }
}
