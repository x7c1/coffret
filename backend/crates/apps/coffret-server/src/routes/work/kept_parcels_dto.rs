use coffret_device::KeptTally;
use serde::Serialize;

/// The parcels this device keeps for the Library, as the Activity panel shows them.
///
/// A fetch keeps every parcel it read until every Entry it covers that this
/// device maps is on the device or witnessed absent, or its Container leaves
/// the current set (spec: PK-21), and nothing else tells a person the
/// directory they are kept in exists: a reader who opens the first page of many books leaves
/// a parcel or two behind for each, which over time is gigabytes.
#[derive(Serialize)]
pub(super) struct KeptParcelsDto {
    /// How many parcels are kept.
    count: u64,
    /// How many bytes their files take on this device's disk.
    bytes: u64,
}

impl KeptParcelsDto {
    pub(super) fn of(tally: KeptTally) -> Self {
        Self {
            count: tally.count,
            bytes: tally.bytes,
        }
    }
}
