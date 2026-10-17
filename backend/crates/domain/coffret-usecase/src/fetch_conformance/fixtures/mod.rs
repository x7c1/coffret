// What the cases are built out of, a file to each thing that has to be arranged.
// The keys are real, derived from one real Master Key, because the whole suite is
// about what a *second* device gets out of Storage — a fixture that faked the
// crypto would prove nothing about that device.

mod catalog;
pub(crate) use catalog::{entry_at, map};

mod files;
pub(crate) use files::{exists, filler, observed, place, scratch_left, unplace};
// The freeze suite reads the fetching device's folder too, and it is in that
// fixture's own in-memory disk for the same reason it is in this one's.
pub(crate) use files::read;

// Where the *source* device's folder is arranged, in the same fake as the
// fetching device's: that device only scans and uploads, and its walk goes
// through `MappedRoots`. The sync suite's helper is what puts files in it.
pub(crate) use crate::sync_conformance::fixtures::write;

mod keys;
pub(crate) use keys::{keys, purpose_key};

mod lose_key;
pub(crate) use lose_key::lose_key;

mod objects;
pub(crate) use objects::container_handle;
pub(super) use objects::{body_start, overwrite, replica_name};

// The parcel arithmetic of PK-19, written out from the rule for the cases that
// count which parcels were read.
mod parcels;
pub(super) use parcels::{parcel_count, parcel_range, parcels_overlapping};

mod plant;
pub(super) use plant::{plant, Planted, OLDER};

mod runs;
pub(crate) use runs::{at, entry_request, freeze_source, policy, request, sync_source};
