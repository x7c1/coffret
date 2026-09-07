// What the cases are built out of, a file to each thing that has to be arranged.
// The keys are real, derived from one real Master Key, because the whole suite is
// about what a *second* device gets out of Storage — a fixture that faked the
// crypto would prove nothing about that device.

mod catalog;
pub(super) use catalog::{entry_at, map};

mod files;
pub(super) use files::{exists, filler, observed, place, scratch_left, unplace};
// The freeze suite reads the fetching device's folder too, and it is a real one
// there for the same reason it is real here.
pub(crate) use files::read;

// Where the *source* device's folder is arranged, which is not a real one at
// all: that device only scans and uploads, and its walk goes through
// `MappedRoots`. The sync suite's helper is what puts files in it.
pub(super) use crate::sync_conformance::fixtures::write;

mod keys;
pub(super) use keys::keys;

mod lose_key;
pub(crate) use lose_key::lose_key;

mod objects;
pub(crate) use objects::container_handle;
pub(super) use objects::{body_start, overwrite, replica_name};

mod plant;
pub(super) use plant::{plant, Planted, OLDER};

mod runs;
pub(super) use runs::{at, entry_request, freeze_source, request, sync_source};
