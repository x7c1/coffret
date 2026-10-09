use std::ops::Range;

use coffret_format::ChunkSize;
use coffret_model::EntryExtent;

use crate::fetch_conformance::fetch_under_test::suite_parcel_len;

/// How long one chunk's message is: the chunk and its tag (spec: FM-5).
const TAG_LEN: u64 = 16;

/// The plaintext bytes of one chunk at the size the encoder writes (spec: FM-6).
fn chunk_size() -> u64 {
    u64::from(ChunkSize::DEFAULT.get())
}

/// How many chunks one parcel holds at the suite's parcel length: the `n` of
/// PK-19, worked out here from the rule rather than asked of the reader under
/// test.
pub(crate) fn chunks_per_parcel() -> u64 {
    (suite_parcel_len().get() / chunk_size()).max(1)
}

/// The parcels an Entry's extent overlaps, by the arithmetic PK-19 states.
///
/// Written out from the rule rather than read off the format crate, so a case
/// holds the reader to the spec instead of to itself.
pub(crate) fn parcels_overlapping(extent: &EntryExtent) -> Range<u64> {
    let per_parcel = chunks_per_parcel() * chunk_size();
    let first = extent.offset() / per_parcel;
    let last = if extent.size() == 0 {
        first
    } else {
        (extent.end() - 1) / per_parcel
    };
    first..last + 1
}

/// The object byte range parcel `index` of a Container occupies, given where
/// its chunk sequence starts and how long the object is (spec: FM-2, FM-5,
/// PK-19).
pub(crate) fn parcel_range(body_start: u64, object_len: u64, index: u64) -> Range<u64> {
    let message = chunk_size() + TAG_LEN;
    let start = body_start + index * chunks_per_parcel() * message;
    start..(start + chunks_per_parcel() * message).min(object_len)
}

/// How many parcels a Container has, given where its chunk sequence starts and
/// how long the object is (spec: PK-19).
pub(crate) fn parcel_count(body_start: u64, object_len: u64) -> u64 {
    let message = chunk_size() + TAG_LEN;
    (object_len - body_start).div_ceil(chunks_per_parcel() * message)
}
