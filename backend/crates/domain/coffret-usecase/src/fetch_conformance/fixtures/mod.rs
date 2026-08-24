// What the cases are built out of, a file to each thing that has to be arranged.
// The keys are real, derived from one real Master Key, because the whole suite is
// about what a *second* device gets out of Storage — a fixture that faked the
// crypto would prove nothing about that device.

mod catalog;
pub(super) use catalog::{entry_at, map};

mod files;
pub(super) use files::{exists, observed, read, scratch_left, write};

mod keys;
pub(super) use keys::keys;

mod lose_key;
pub(super) use lose_key::lose_key;

mod objects;
pub(super) use objects::{container_handle, overwrite, replica_name};

mod plant;
pub(super) use plant::{plant, Planted, OLDER};

mod runs;
pub(super) use runs::{at, request, sync_source};
